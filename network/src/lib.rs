// impl Trait
#![feature(conservative_impl_trait)]
// Network

extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate screeps_api;
extern crate tokio_core;
extern crate websocket;
// Caching

extern crate app_dirs;
extern crate bincode;
extern crate futures_cpupool;
extern crate rocksdb;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate time;
// Logging

#[macro_use]
extern crate log;

pub mod request;
pub mod event;
pub mod memcache;
pub mod diskcache;
pub mod tokio;

use std::fmt;

pub use request::{LoginDetails, NotLoggedIn, Request, SelectedRooms};
pub use event::{MapCache, MapCacheData, NetworkEvent};
pub use memcache::{ErrorEvent, LoginState, MemCache};
pub use tokio::Handler as TokioHandler;

/// The backend connection handler for handling requests. Interface for `memcache` module to use.
pub trait ScreepsConnection {
    /// Send a request. Any and all errors will be returned in the future via poll()
    fn send(&mut self, r: Request);

    /// Get the next available event if any, or return None if nothing new has happened.
    ///
    /// Should not error if any threads have disconnected.
    fn poll(&mut self) -> Option<NetworkEvent>;
}

/// An error for the `Notify` trait to output.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Disconnected;

pub trait Notify: Clone + Send + 'static {
    fn wakeup(&self) -> Result<(), Disconnected>;
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct ConnectionSettings {
    pub username: String,
    pub password: String,
    pub shard: Option<String>,
}

impl ConnectionSettings {
    pub fn new<T: Into<Option<String>>>(username: String, password: String, shard: T) -> ConnectionSettings {
        ConnectionSettings {
            username: username,
            password: password,
            shard: shard.into(),
        }
    }
}

impl fmt::Debug for ConnectionSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ConnectionSettings")
            .field("username", &self.username)
            .field("password", &"<hidden>")
            .field("shard", &self.shard)
            .finish()
    }
}
