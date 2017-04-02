//! Handling network connections in a separate thread.
//!
//! This currently only supports a single threaded thread, but work may be done to allow multiple concurrent network
//! connections.

mod cache;
mod request;
mod single_threaded;

pub use self::request::{Request, NetworkEvent};
pub use self::cache::{NetCache, LoginState};

pub use self::single_threaded::Handler as ThreadedHandler;

/// The backend connection handler for handling requests.
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

/// Error for not being logged in, and trying to send a query requiring authentication.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct NotLoggedIn;
