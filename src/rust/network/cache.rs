use std::ops;
use std::borrow::Cow;

use screeps_api;
use time::{self, Duration};

use super::{Request, NetworkEvent, ScreepsConnection, NotLoggedIn};

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

pub struct NetCache {
    last_cache_clear: time::Tm,
    login: TimeoutValue<()>,
    my_info: TimeoutValue<screeps_api::MyInfo>,
}

pub struct ActiveCache<'a, T: ScreepsConnection + 'a> {
    inner: &'a mut NetCache,
    handler: &'a mut T,
}

impl<'a, T: ScreepsConnection> ops::Deref for ActiveCache<'a, T> {
    type Target = NetCache;
    fn deref(&self) -> &NetCache {
        &self.inner
    }
}
impl<'a, T: ScreepsConnection> ops::DerefMut for ActiveCache<'a, T> {
    fn deref_mut(&mut self) -> &mut NetCache {
        &mut self.inner
    }
}

impl NetCache {
    pub fn new() -> Self {
        NetCache {
            last_cache_clear: time::now_utc(),
            login: TimeoutValue::default(),
            my_info: TimeoutValue::default(),
        }
    }

    fn event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Login { username_requested: _, result } => {
                if let Err(e) = self.login.event(result) {
                    error!("login failed: {}", e);
                }
            }
            NetworkEvent::MyInfo(result) => {
                if let Err(e) = self.my_info.event(result) {
                    error!("my_info failed: {}", e);
                }
            }
        }
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
    pub fn align<'a, T: ScreepsConnection>(&'a mut self, handler: &'a mut T) -> ActiveCache<'a, T> {
        while let Some(evt) = handler.poll() {
            debug!("[cache] Got event {:?}", evt);
            self.event(evt);
        }

        ActiveCache {
            inner: self,
            handler: handler,
        }
    }
}

impl<'a, T: ScreepsConnection> ActiveCache<'a, T> {
    pub fn login<'b, U, P>(&mut self, username: U, password: P)
        where U: Into<Cow<'b, str>>,
              P: Into<Cow<'b, str>>
    {
        self.handler
            .send(Request::login(username, password))
            .expect("expected login call not to result in not-logged-in error")
    }

    pub fn my_info(&mut self) -> Result<Option<&screeps_api::MyInfo>, NotLoggedIn> {
        if self.my_info.should_request(Some(Duration::minutes(10)), Duration::seconds(90)) {
            self.handler.send(Request::MyInfo)?;
            self.my_info.requested();
        }
        Ok(self.my_info.get())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum LoginState {
    NotLoggedIn,
    TryingToLogin,
    LoggedIn,
}
