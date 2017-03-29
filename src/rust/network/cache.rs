use std::ops;

use screeps_api;
use time::{self, Duration};

use super::{NetworkEvent, NetworkRequests};
use super::request::Request;

#[derive(Copy, Clone, Debug)]
struct TimeoutValue<T> {
    /// T retrieved, time it was retrieved.
    value: Option<(T, time::Timespec)>,
    /// Last call made to server, set to None when value is received or when error is.
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

    fn requested(&mut self) { self.last_send = Some(time::get_time()); }

    /// Gets the value if there is any. This is independent of whether or not we should make a new request.
    fn get(&self) -> Option<&T> { self.value.as_ref().map(|tuple| &tuple.0) }
}

#[derive(Default)]
struct InnerCache {
    login: TimeoutValue<()>,
    my_info: TimeoutValue<screeps_api::MyInfo>,
}

pub struct NetCache {
    last_cache_clear: time::Tm,
    cache: InnerCache,
}

pub struct CallableCache<'a> {
    inner: &'a mut NetCache,
    handler: &'a mut NetworkRequests,
}

impl<'a> ops::Deref for CallableCache<'a> {
    type Target = NetCache;
    fn deref(&self) -> &NetCache { &self.inner }
}
impl<'a> ops::DerefMut for CallableCache<'a> {
    fn deref_mut(&mut self) -> &mut NetCache { &mut self.inner }
}


impl InnerCache {
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
}

impl NetCache {
    pub fn new() -> Self {
        NetCache {
            last_cache_clear: time::now_utc(),
            cache: InnerCache::default(),
        }
    }

    pub fn login_state(&self) -> LoginState {
        match self.cache.login.get() {
            Some(_) => LoginState::LoggedIn,
            None => {
                match self.cache.login.should_request(None, Duration::seconds(90)) {
                    false => LoginState::TryingToLogin,
                    true => LoginState::NotLoggedIn,
                }
            }
        }
    }

    pub fn align<'a>(&'a mut self, handler: &'a mut NetworkRequests) -> CallableCache<'a> {
        while let Some(evt) = handler.poll() {
            self.cache.event(evt);
        }

        CallableCache {
            inner: self,
            handler: handler,
        }
    }
}

impl<'a> CallableCache<'a> {
    pub fn my_info(&mut self) -> Option<&screeps_api::MyInfo> {
        if self.cache.my_info.should_request(Some(Duration::minutes(10)), Duration::seconds(90)) {
            self.handler.send(Request::MyInfo);
            self.cache.my_info.requested();
        }
        self.cache.my_info.get()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum LoginState {
    NotLoggedIn,
    TryingToLogin,
    LoggedIn,
}
