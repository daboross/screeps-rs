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

pub use request::{LoginDetails, NotLoggedIn, Request, SelectedRooms};
pub use event::{MapCache, MapCacheData, NetworkEvent};
pub use memcache::{ErrorEvent, LoginState, MemCache};
pub use tokio::Handler as TokioHandler;

/// The backend connection handler for handling requests. Interface for `memcache` module to use.
pub trait ScreepsConnection {
    /// Send a request. The only error condition should be that the connection is currently not logged in.
    ///
    /// NotLoggedIn errors may instead be sent back as NetworkEvents, if using this be sure to account for both.
    fn send(&mut self, r: Request) -> Result<(), NotLoggedIn>;

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
