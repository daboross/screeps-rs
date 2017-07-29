use screeps_api;

use request::{Request, SelectedRooms, LoginDetails};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum HttpRequest {
    Login { details: LoginDetails },
    MyInfo,
    RoomTerrain { room_name: screeps_api::RoomName },
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum WebsocketRequest {
    SetMapSubscribes { rooms: SelectedRooms },
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum GenericRequest {
    Http(HttpRequest),
    Websocket(WebsocketRequest),
}

impl From<Request> for GenericRequest {
    fn from(r: Request) -> Self {
        match r {
            Request::Login { details } => GenericRequest::Http(HttpRequest::Login { details: details }),
            Request::MyInfo => GenericRequest::Http(HttpRequest::MyInfo),
            Request::RoomTerrain { room_name } => {
                GenericRequest::Http(HttpRequest::RoomTerrain { room_name: room_name })
            }
            Request::SetMapSubscribes { rooms } => {
                GenericRequest::Websocket(WebsocketRequest::SetMapSubscribes { rooms: rooms })
            }
        }
    }
}

impl Into<Request> for HttpRequest {
    fn into(self) -> Request {
        match self {
            HttpRequest::Login { details } => Request::Login { details: details },
            HttpRequest::MyInfo => Request::MyInfo,
            HttpRequest::RoomTerrain { room_name } => Request::RoomTerrain { room_name: room_name },
        }
    }
}

impl Into<Request> for WebsocketRequest {
    fn into(self) -> Request {
        match self {
            WebsocketRequest::SetMapSubscribes { rooms } => Request::SetMapSubscribes { rooms: rooms },
        }
    }
}
