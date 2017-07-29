use std::fmt;

use {screeps_api, websocket};

pub use self::memory::{MemCache, NetworkedMemCache};

mod memory;

pub enum ErrorEvent {
    NotLoggedIn,
    ErrorOccurred(screeps_api::Error),
    WebsocketError(websocket::WebSocketError),
    WebsocketParse(screeps_api::websocket::parsing::ParseError),
    RoomViewError(String), // TODO: granularity here.
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

impl From<screeps_api::websocket::parsing::ParseError> for ErrorEvent {
    fn from(err: screeps_api::websocket::parsing::ParseError) -> ErrorEvent {
        ErrorEvent::WebsocketParse(err)
    }
}

impl ErrorEvent {
    fn room_view(data: String) -> Self {
        ErrorEvent::RoomViewError(data)
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
            ErrorEvent::WebsocketError(ref e) => e.fmt(f),
            ErrorEvent::WebsocketParse(ref e) => e.fmt(f),
            ErrorEvent::RoomViewError(ref e) => e.fmt(f),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum LoginState {
    NotLoggedIn,
    TryingToLogin,
    LoggedIn,
}
