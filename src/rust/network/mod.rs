//! Handling network connections in a separate thread.
//!
//! This currently only supports a single threaded thread, but work may be done to allow multiple concurrent network
//! connections.

pub mod cache;
pub mod types;
pub mod request;
mod tokio;

use std::sync::Arc;
use std::fmt;

pub use self::request::{Request, NetworkEvent};
pub use self::cache::{LoginState, ErrorEvent, MemCache, NetworkedMemCache};
pub use self::types::{SelectedRooms, MapCache, MapCacheData};

pub use self::tokio::Handler as ThreadedHandler;

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

/// Login username/password.
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct LoginDetails {
    inner: Arc<(String, String)>,
}

impl LoginDetails {
    /// Creates a new login detail struct.
    pub fn new(username: String, password: String) -> Self {
        LoginDetails { inner: Arc::new((username, password)) }
    }

    /// Gets the username.
    pub fn username(&self) -> &str {
        &self.inner.0
    }

    /// Gets the password.
    pub fn password(&self) -> &str {
        &self.inner.1
    }
}

impl fmt::Debug for LoginDetails {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LoginDetails")
            .field("username", &self.username())
            .field("password", &"<redacted>")
            .finish()
    }
}
