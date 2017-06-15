use std::borrow::Cow;
use std::rc::Rc;
use std::cell::Ref;
use std::cell::RefCell;
use std::collections::HashMap;

use screeps_api::{self, RoomName};
use time::{self, Duration};

use network::{LoginState, ErrorEvent, MapCacheData, SelectedRooms, NetworkEvent, Request, ScreepsConnection};

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
            Some((_, last_request)) => {
                match cache_for {
                    Some(cache_for) => {
                        if last_request + cache_for > now {
                            false
                        } else {
                            match self.last_send {
                                Some(send_time) => send_time + timeout_for_request < now,
                                None => true,
                            }
                        }
                    }
                    None => false,
                }
            }
            None => {
                match self.last_send {
                    Some(t) => t + timeout_for_request < now,
                    None => true,
                }
            }
        }
    }

    fn requested(&mut self) {
        self.last_send = Some(time::get_time());
    }

    /// Gets the value if there is any. This is independent of whether or not we should make a new request.
    fn get(&self) -> Option<&T> {
        self.value.as_ref().map(|tuple| &tuple.0)
    }
}

#[derive(Default, Debug)]
pub struct MemCache {
    login: TimeoutValue<()>,
    my_info: TimeoutValue<screeps_api::MyInfo>,
    rooms: Rc<RefCell<MapCacheData>>,
    requested_rooms: HashMap<RoomName, time::Timespec>,
    last_requested_room_info: Option<SelectedRooms>,
}

pub struct NetworkedMemCache<'a, T: ScreepsConnection + 'a, F: FnMut(ErrorEvent) + 'a> {
    cache: &'a mut MemCache,
    handler: &'a mut T,
    error_callback: F,
}

impl MemCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn event(&mut self, event: NetworkEvent) -> Result<(), ErrorEvent> {
        match event {
            NetworkEvent::Login { username: _, result } => self.login.event(result)?,
            NetworkEvent::MyInfo { result } => self.my_info.event(result)?,
            NetworkEvent::RoomTerrain { room_name, result } => {
                let terrain = result?;
                self.rooms
                    .borrow_mut()
                    .terrain
                    .insert(room_name, (time::get_time(), terrain));
            }
        }

        Ok(())
    }

    pub fn login_state(&self) -> LoginState {
        match self.login.get() {
            Some(_) => LoginState::LoggedIn,
            None => {
                match self.login.should_request(None, Duration::seconds(90)) {
                    false => LoginState::TryingToLogin,
                    true => LoginState::NotLoggedIn,
                }
            }
        }
    }

    /// TODO: this method should return 'failure events' which the UI can then cache and add to a notification list.
    pub fn align<'a, T, F>(&'a mut self, handler: &'a mut T, mut error_callback: F) -> NetworkedMemCache<'a, T, F>
        where T: ScreepsConnection,
              F: FnMut(ErrorEvent)
    {
        while let Some(evt) = handler.poll() {
            debug!("[cache] Got event {:?}", evt);
            if let Err(e) = self.event(evt) {
                error_callback(e);
            }
        }

        NetworkedMemCache {
            cache: self,
            handler: handler,
            error_callback: error_callback,
        }
    }
}

impl<'a, C: ScreepsConnection, F: FnMut(ErrorEvent)> NetworkedMemCache<'a, C, F> {
    pub fn login<'b, U, P>(&mut self, username: U, password: P)
        where U: Into<Cow<'b, str>>,
              P: Into<Cow<'b, str>>
    {
        self.handler
            .send(Request::login(username, password))
            .expect("expected login call not to result in not-logged-in error")
    }

    pub fn my_info(&mut self) -> Option<&screeps_api::MyInfo> {
        let holder = &mut self.cache.my_info;
        if holder.should_request(Some(Duration::minutes(10)), Duration::seconds(90)) {
            if let Err(e) = self.handler.send(Request::MyInfo) {
                (self.error_callback)(e.into())
            }
            holder.requested();
        }

        holder.get()
    }

    pub fn view_rooms(&mut self, rooms: SelectedRooms) -> &Rc<RefCell<MapCacheData>> {
        if Some(rooms) != self.cache.last_requested_room_info {
            let borrowed = Ref::map(self.cache.rooms.borrow(), |cache| &cache.terrain);
            let rerequest_if_before = time::get_time() - Duration::seconds(90);
            for room_name in rooms {
                if !borrowed.contains_key(&room_name) {
                    let resend = match self.cache
                        .requested_rooms
                        .get(&room_name) {
                        Some(v) => v < &rerequest_if_before,
                        None => true,
                    };

                    if resend {
                        if let Err(e) = self.handler.send(Request::room_terrain(room_name)) {
                            (self.error_callback)(e.into())
                        }
                    }
                }
            }
        }
        &self.cache.rooms
    }
}
