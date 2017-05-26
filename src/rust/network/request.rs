use std::borrow::Cow;

use screeps_api;

use self::Request::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Request<'a> {
    Login {
        username: Cow<'a, str>,
        password: Cow<'a, str>,
    },
    MyInfo,
    RoomTerrain { room_name: screeps_api::RoomName },
}

impl<'a> Request<'a> {
    pub fn login<T, U>(username: T, password: U) -> Self
        where T: Into<Cow<'a, str>>,
              U: Into<Cow<'a, str>>
    {
        Login {
            username: username.into(),
            password: password.into(),
        }
    }

    pub fn into_static(self) -> Request<'static> {
        match self {
            Login { username, password } => {
                Login {
                    username: username.into_owned().into(),
                    password: password.into_owned().into(),
                }
            }
            MyInfo => MyInfo,
            RoomTerrain { room_name } => RoomTerrain { room_name: room_name },
        }
    }

    pub fn exec_with<T>(self, client: &mut screeps_api::API<T>) -> NetworkEvent
        where T: screeps_api::HyperClient
    {
        match self {
            Login { username, password } => {
                let result = client.login(username.as_ref(), password);
                NetworkEvent::Login {
                    username: username.into_owned(),
                    result: result,
                }
            }
            MyInfo => {
                let result = client.my_info();
                NetworkEvent::MyInfo { result: result }
            }
            RoomTerrain { room_name } => {
                let result = client.room_terrain(room_name.to_string());
                NetworkEvent::RoomTerrain {
                    room_name: room_name,
                    result: result,
                }
            }
        }
    }
}

impl Request<'static> {
    pub fn my_info() -> Self {
        Request::MyInfo
    }

    pub fn room_terrain(room_name: screeps_api::RoomName) -> Self {
        Request::RoomTerrain { room_name: room_name }
    }
}

#[derive(Debug)]
pub enum NetworkEvent {
    Login {
        username: String,
        result: screeps_api::Result<()>,
    },
    MyInfo { result: screeps_api::Result<screeps_api::MyInfo>, },
    RoomTerrain {
        room_name: screeps_api::RoomName,
        result: screeps_api::Result<screeps_api::RoomTerrain>,
    },
}
