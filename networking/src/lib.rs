// impl Trait
#![feature(conservative_impl_trait)]
// Network
extern crate futures;
extern crate tokio_core;
extern crate hyper;
extern crate hyper_tls;
extern crate websocket;
extern crate screeps_api;
// Caching
extern crate time;
extern crate bincode;
extern crate rocksdb;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate app_dirs;
extern crate futures_cpupool;
// Logging
#[macro_use]
extern crate log;

pub mod request;
pub mod event;
pub mod memcache;
pub mod diskcache;
pub mod tokio;

pub use request::{Request, NotLoggedIn, LoginDetails, SelectedRooms};
pub use event::{NetworkEvent, MapCache, MapCacheData};
pub use memcache::{MemCache, LoginState, ErrorEvent};
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
