use std::borrow::Cow;

use screeps_api;

use self::Request::*;

use super::LoginDetails;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Request {
    Login { details: LoginDetails },
    MyInfo,
    RoomTerrain { room_name: screeps_api::RoomName },
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
}


impl NetworkEvent {
    pub fn error(&self) -> Option<&screeps_api::Error> {
        match *self {
            NetworkEvent::Login { ref result, .. } => result.as_ref().err(),
            NetworkEvent::MyInfo { ref result, .. } => result.as_ref().err(),
            NetworkEvent::RoomTerrain { ref result, .. } => result.as_ref().err(),
        }
    }
}
