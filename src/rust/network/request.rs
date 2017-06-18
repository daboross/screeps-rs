use std::borrow::Cow;

use {screeps_api, websocket};

use network::SelectedRooms;

use self::Request::*;

use super::LoginDetails;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Request {
    Login { details: LoginDetails },
    MyInfo,
    RoomTerrain { room_name: screeps_api::RoomName },
    SetMapSubscribes { rooms: SelectedRooms },
}

impl Request {
    pub fn login<'a, T, U>(username: T, password: U) -> Self
        where T: Into<Cow<'a, str>>,
              U: Into<Cow<'a, str>>
    {
        Login { details: LoginDetails::new(username.into().into_owned(), password.into().into_owned()) }
    }

    pub fn login_with_details(details: LoginDetails) -> Self {
        Login { details: details }
    }

    pub fn my_info() -> Self {
        Request::MyInfo
    }

    pub fn room_terrain(room_name: screeps_api::RoomName) -> Self {
        RoomTerrain { room_name: room_name }
    }

    pub fn subscribe_map_view(rooms: SelectedRooms) -> Self {
        SetMapSubscribes { rooms: rooms }
    }
}

#[derive(Debug)]
pub enum NetworkEvent {
    Login {
        username: String,
        result: Result<(), screeps_api::Error>,
    },
    MyInfo { result: Result<screeps_api::MyInfo, screeps_api::Error>, },
    RoomTerrain {
        room_name: screeps_api::RoomName,
        result: Result<screeps_api::TerrainGrid, screeps_api::Error>,
    },
    WebsocketHttpError { error: screeps_api::Error },
    WebsocketError { error: websocket::WebSocketError },
    WebsocketParseError { error: screeps_api::websocket::parsing::ParseError, },
    MapView {
        room_name: screeps_api::RoomName,
        result: screeps_api::websocket::RoomMapViewUpdate,
    },
}


impl NetworkEvent {
    pub fn error(&self) -> Option<&screeps_api::Error> {
        match *self {
            NetworkEvent::Login { ref result, .. } => result.as_ref().err(),
            NetworkEvent::MyInfo { ref result, .. } => result.as_ref().err(),
            NetworkEvent::RoomTerrain { ref result, .. } => result.as_ref().err(),
            NetworkEvent::WebsocketHttpError { ref error } => Some(error),
            NetworkEvent::MapView { .. } |
            NetworkEvent::WebsocketError { .. } |
            NetworkEvent::WebsocketParseError { .. } => None,
        }
    }
}
