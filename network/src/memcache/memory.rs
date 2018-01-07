use std::rc::Rc;
use std::sync::Arc;
use std::cell::Ref;
use std::cell::RefCell;
use std::collections::HashMap;

use screeps_api::{self, RoomName};
use time::{self, Duration};

use super::{ErrorEvent, LoginState};
use event::{MapCacheData, NetworkEvent};
use request::{Request, SelectedRooms};
use {ConnectionSettings, ScreepsConnection};

#[derive(Copy, Clone, Debug)]
struct TimeoutValue<T> {
    /// T retrieved, time it was retrieved.
    value: Option<(T, time::Timespec)>,
    /// Last call made to server, set to None when a value or error is received.
    last_send: Option<time::Timespec>,
}

impl<T> Default for TimeoutValue<T> {
    fn default() -> Self {
        TimeoutValue {
            value: None,
            last_send: None,
        }
    }
}

impl<T> TimeoutValue<T> {
    fn event<E>(&mut self, result: Result<T, E>) -> Result<(), E> {
        self.last_send = None;
        match result {
            Ok(v) => {
                self.value = Some((v, time::get_time()));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Gets whether or not we should launch a request for this resource. This is somewhat
    /// independent of whether we already have an old copy or not.
    ///
    /// If cache_for is None, values will be held indefinitely without re-requesting.
    fn should_request(&self, cache_for: Option<Duration>, timeout_for_request: Duration) -> bool {
        let now = time::get_time();

        match self.value {
            Some((_, last_request)) => match cache_for {
                Some(cache_for) => if last_request + cache_for > now {
                    false
                } else {
                    match self.last_send {
                        Some(send_time) => send_time + timeout_for_request < now,
                        None => true,
                    }
                },
                None => false,
            },
            None => match self.last_send {
                Some(t) => t + timeout_for_request < now,
                None => true,
            },
        }
    }

    fn requested(&mut self) {
        self.last_send = Some(time::get_time());
    }

    /// Gets the value if there is any. This is independent of whether or not we should make a new request.
    fn get(&self) -> Option<&T> {
        self.value.as_ref().map(|tuple| &tuple.0)
    }

    /// Resets the value to None.
    fn reset(&mut self) {
        self.value = None;
        self.last_send = None;
    }
}

#[derive(Default, Debug)]
pub struct MemCache {
    login: TimeoutValue<()>,
    my_info: TimeoutValue<screeps_api::MyInfo>,
    shard_list: TimeoutValue<Option<Vec<screeps_api::ShardInfo>>>,
    rooms: Rc<RefCell<MapCacheData>>,
    requested_rooms: HashMap<RoomName, time::Timespec>,
    last_requested_room_info: Option<SelectedRooms>,
    last_requested_focus_room: Option<RoomName>,
}

pub struct NetworkedMemCache<'a, T: ScreepsConnection + 'a> {
    cache: &'a mut MemCache,
    handler: &'a mut T,
}

impl MemCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn event(&mut self, event: NetworkEvent) -> Result<(), ErrorEvent> {
        match event {
            NetworkEvent::Login {
                username: _,
                result,
            } => self.login.event(result)?,
            NetworkEvent::MyInfo { result } => self.my_info.event(result)?,
            NetworkEvent::ShardList { result } => self.shard_list.event(result)?,
            NetworkEvent::RoomTerrain { room_name, result } => {
                let terrain = match result {
                    Ok(terrain) => Some(terrain),
                    Err(err) => {
                        if let &screeps_api::ErrorKind::Api(screeps_api::error::ApiError::InvalidRoom) = err.kind() {
                            None
                        } else {
                            return Err(err.into());
                        }
                    }
                };
                self.rooms
                    .borrow_mut()
                    .terrain
                    .insert(room_name, (time::get_time(), terrain));
            }
            NetworkEvent::MapView { room_name, result } => {
                self.rooms
                    .borrow_mut()
                    .map_views
                    .insert(room_name, (time::get_time(), result));
            }
            NetworkEvent::RoomView { room_name, result } => {
                use serde_json;
                use std::collections::hash_map::Entry::*;

                let mut data = self.rooms.borrow_mut();

                let mut new_detail_view = None;

                match data.detail_view.as_mut() {
                    Some(&mut (name, ref mut map)) if name == room_name => {
                        for (id, obj_update) in result.objects.into_iter() {
                            if obj_update.is_null() {
                                map.remove(&id);
                            } else {
                                match map.entry(id.clone()) {
                                    Occupied(entry) => {
                                        let obj_data = entry.into_mut();

                                        obj_data.update(obj_update.clone()).map_err(|e| {
                                            ErrorEvent::room_view(format!(
                                                "update for id {} in room {} did not \
                                                 parse: existing value: {:?}, failed \
                                                 update: {:?}, error: {}",
                                                id, room_name, obj_data, obj_update, e
                                            ))
                                        })?;
                                    }
                                    Vacant(entry) => {
                                        entry.insert(serde_json::from_value(obj_update.clone()).map_err(|e| {
                                            ErrorEvent::room_view(format!(
                                                "data for id {} in room {} did not \
                                                 parse: failed json: {:?}, error: {}",
                                                id, room_name, obj_update, e
                                            ))
                                        })?);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        let new_map = result
                            .objects
                            .into_iter()
                            .map(|(id, obj_json)| {
                                let data = serde_json::from_value(obj_json.clone()).map_err(|e| {
                                    ErrorEvent::room_view(format!(
                                        "data for id {} in room {} did not parse: \
                                         failed json: {:?}, error: {}",
                                        id, room_name, obj_json, e
                                    ))
                                })?;
                                Ok((id, data))
                            })
                            .collect::<Result<HashMap<_, _>, ErrorEvent>>()?;

                        new_detail_view = Some(new_map);
                    }
                }

                if let Some(view) = new_detail_view {
                    data.detail_view = Some((room_name, view));
                }
            }
            NetworkEvent::WebsocketError { error } => return Err(ErrorEvent::WebsocketError(error)),
            NetworkEvent::WebsocketHttpError { error } => return Err(ErrorEvent::ErrorOccurred(error)),
            NetworkEvent::WebsocketParseError { error } => return Err(ErrorEvent::WebsocketParse(error)),
        }

        Ok(())
    }

    pub fn login_state(&self) -> LoginState {
        match self.login.get() {
            Some(_) => LoginState::LoggedIn,
            None => match self.login.should_request(None, Duration::seconds(90)) {
                false => LoginState::TryingToLogin,
                true => LoginState::NotLoggedIn,
            },
        }
    }

    pub fn align<'a, T, F, E>(
        &'a mut self,
        handler: &'a mut T,
        mut error_callback: F,
        mut additional_event_receiver: E,
    ) -> NetworkedMemCache<'a, T>
    where
        T: ScreepsConnection,
        F: FnMut(ErrorEvent),
        E: FnMut(&NetworkEvent),
    {
        while let Some(evt) = handler.poll() {
            debug!("[cache] Got event {:?}", evt);
            additional_event_receiver(&evt);
            if let Err(e) = self.event(evt) {
                if let ErrorEvent::NotLoggedIn = e {
                    self.login.reset();
                }
                error_callback(e);
            }
        }

        NetworkedMemCache {
            cache: self,
            handler: handler,
        }
    }
}

impl<'a, C: ScreepsConnection> NetworkedMemCache<'a, C> {
    pub fn login(&mut self) {
        self.handler.send(Request::login());
    }

    pub fn login_state(&self) -> LoginState {
        self.cache.login_state()
    }

    pub fn update_settings(&mut self, settings: ConnectionSettings) {
        self.handler.send(Request::ChangeSettings {
            settings: Arc::new(settings),
        })
    }

    pub fn my_info(&mut self) -> Option<&screeps_api::MyInfo> {
        let holder = &mut self.cache.my_info;
        if holder.should_request(Some(Duration::minutes(10)), Duration::seconds(90)) {
            self.handler.send(Request::MyInfo);
            holder.requested();
        }

        holder.get()
    }

    pub fn shard_list(&mut self) -> Option<Option<&[screeps_api::ShardInfo]>> {
        let holder = &mut self.cache.shard_list;
        if holder.should_request(Some(Duration::hours(6)), Duration::seconds(90)) {
            self.handler.send(Request::ShardList);
            holder.requested();
        }

        holder.get().map(|o| o.as_ref().map(AsRef::as_ref))
    }

    pub fn view_rooms(&mut self, rooms: SelectedRooms, focused: Option<RoomName>) -> &Rc<RefCell<MapCacheData>> {
        if Some(rooms) != self.cache.last_requested_room_info {
            let borrowed = Ref::map(self.cache.rooms.borrow(), |cache| &cache.terrain);
            let rerequest_if_before = time::get_time() - Duration::seconds(90);
            for room_name in rooms {
                if !borrowed.contains_key(&room_name) {
                    let resend = match self.cache.requested_rooms.get(&room_name) {
                        Some(v) => v < &rerequest_if_before,
                        None => true,
                    };

                    if resend {
                        self.cache.requested_rooms.insert(room_name, time::get_time());
                        self.handler.send(Request::room_terrain(room_name));
                    }
                }
            }
            self.handler.send(Request::subscribe_map_view(rooms));
        }
        if focused != self.cache.last_requested_focus_room {
            self.handler.send(Request::focus_room(focused));
        }
        &self.cache.rooms
    }
}
