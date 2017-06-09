use std::fmt;

use screeps_api;

pub use self::memory::{MemCache, NetworkedMemCache};

mod memory;
pub mod disk;

pub enum ErrorEvent {
    NotLoggedIn,
    ErrorOccurred(screeps_api::Error),
}

impl From<screeps_api::NoToken> for ErrorEvent {
    fn from(_: screeps_api::NoToken) -> ErrorEvent {
        ErrorEvent::NotLoggedIn
    }
}

impl From<super::NotLoggedIn> for ErrorEvent {
    fn from(_: super::NotLoggedIn) -> ErrorEvent {
        ErrorEvent::NotLoggedIn
    }
}

impl From<screeps_api::Error> for ErrorEvent {
    fn from(err: screeps_api::Error) -> ErrorEvent {
        ErrorEvent::ErrorOccurred(err)
    }
}

impl fmt::Display for ErrorEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorEvent::NotLoggedIn => {
                write!(f,
                       "network connection attempted that is not available without logging in.")
            }
            ErrorEvent::ErrorOccurred(ref e) => e.fmt(f),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum LoginState {
    NotLoggedIn,
    TryingToLogin,
    LoggedIn,
}
