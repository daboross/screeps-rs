use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;

use screeps_api;
use time::{self, Duration};

use super::{LoginState, ErrorEvent};
use super::super::{Request, NetworkEvent, ScreepsConnection};

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

pub struct MemCache {
    login: TimeoutValue<()>,
    my_info: TimeoutValue<screeps_api::MyInfo>,
    terrain: HashMap<screeps_api::RoomName, TimeoutValue<Rc<screeps_api::endpoints::room_terrain::TerrainGrid>>>,
}

pub struct NetworkedMemCache<'a, T: ScreepsConnection + 'a, F: FnMut(ErrorEvent) + 'a> {
    cache: &'a mut MemCache,
    handler: &'a mut T,
    error_callback: F,
}

impl MemCache {
    pub fn new() -> Self {
        MemCache {
            login: TimeoutValue::default(),
            my_info: TimeoutValue::default(),
            terrain: HashMap::new(),
        }
    }

    fn event(&mut self, event: NetworkEvent) -> Option<ErrorEvent> {
        let err = match event {
            NetworkEvent::Login { username: _, result } => self.login.event(result).err(),
            NetworkEvent::MyInfo { result } => self.my_info.event(result).err(),
            NetworkEvent::RoomTerrain { room_name, result } => {
                self.terrain
                    .entry(room_name)
                    .or_insert_with(TimeoutValue::default)
                    .event(result.map(Rc::new))
                    .err()
            }
        };

        err.map(ErrorEvent::ErrorOccurred)
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
            if let Some(e) = self.event(evt) {
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

    pub fn room_terrain(&mut self,
                        room_name: screeps_api::RoomName)
                        -> Option<&Rc<screeps_api::endpoints::room_terrain::TerrainGrid>> {
        let holder = self.cache.terrain.entry(room_name).or_insert_with(TimeoutValue::default);

        if holder.should_request(Some(Duration::minutes(360)), Duration::seconds(30)) {
            if let Err(e) = self.handler.send(Request::room_terrain(room_name)) {
                (self.error_callback)(e.into())
            }
            holder.requested();
        }

        holder.get()
    }
}
