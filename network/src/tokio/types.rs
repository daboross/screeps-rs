use std::sync::Arc;

use screeps_api;

use request::{Request, SelectedRooms};
use ConnectionSettings;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum HttpRequest {
    Login,
    MyInfo,
    RoomTerrain { room_name: screeps_api::RoomName },
    ChangeSettings { settings: Arc<ConnectionSettings> },
    Exit,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum WebsocketRequest {
    SetMapSubscribes { rooms: SelectedRooms },
    SetFocusRoom { room: Option<screeps_api::RoomName> },
    ChangeSettings { settings: Arc<ConnectionSettings> },
    Exit,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum GenericRequest {
    Http(HttpRequest),
    Websocket(WebsocketRequest),
    Both(HttpRequest, WebsocketRequest),
}

impl From<Request> for GenericRequest {
    fn from(r: Request) -> Self {
        match r {
            Request::Login => GenericRequest::Http(HttpRequest::Login),
            Request::MyInfo => GenericRequest::Http(HttpRequest::MyInfo),
            Request::RoomTerrain { room_name } => GenericRequest::Http(HttpRequest::RoomTerrain {
                room_name: room_name,
            }),
            Request::SetMapSubscribes { rooms } => {
                GenericRequest::Websocket(WebsocketRequest::SetMapSubscribes { rooms: rooms })
            }
            Request::SetFocusRoom { room } => GenericRequest::Websocket(WebsocketRequest::SetFocusRoom { room: room }),
            Request::ChangeSettings { settings } => GenericRequest::Both(
                HttpRequest::ChangeSettings {
                    settings: settings.clone(),
                },
                WebsocketRequest::ChangeSettings { settings: settings },
            ),
            Request::Exit => GenericRequest::Both(HttpRequest::Exit, WebsocketRequest::Exit),
        }
    }
}

impl Into<Request> for HttpRequest {
    fn into(self) -> Request {
        match self {
            HttpRequest::Login => Request::Login,
            HttpRequest::MyInfo => Request::MyInfo,
            HttpRequest::RoomTerrain { room_name } => Request::RoomTerrain {
                room_name: room_name,
            },
            HttpRequest::ChangeSettings { settings } => Request::ChangeSettings { settings: settings },
            HttpRequest::Exit => Request::Exit,
        }
    }
}

impl Into<Request> for WebsocketRequest {
    fn into(self) -> Request {
        match self {
            WebsocketRequest::SetMapSubscribes { rooms } => Request::SetMapSubscribes { rooms: rooms },
            WebsocketRequest::SetFocusRoom { room } => Request::SetFocusRoom { room: room },
            WebsocketRequest::ChangeSettings { settings } => Request::ChangeSettings { settings: settings },
            WebsocketRequest::Exit => Request::Exit,
        }
    }
}
