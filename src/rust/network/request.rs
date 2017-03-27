use std::borrow::Cow;

use screeps_api;

use super::NetworkEvent;

use self::Request::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Request<'a> {
    Login {
        username: Cow<'a, str>,
        password: Cow<'a, str>,
    },
    MyInfo,
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

    pub fn my_info() -> Request<'static> { Request::MyInfo }

    pub fn into_static(self) -> Request<'static> {
        match self {
            Login { username, password } => {
                Login {
                    username: username.into_owned().into(),
                    password: password.into_owned().into(),
                }
            }
            MyInfo => MyInfo,
        }
    }

    pub fn exec_with<T>(self, client: &mut screeps_api::API<T>) -> NetworkEvent
        where T: screeps_api::HyperClient
    {
        match self {
            Login { username, password } => {
                let result = client.login(username.as_ref(), password);
                NetworkEvent::Login {
                    username_requested: username.into_owned(),
                    result: result,
                }
            }
            MyInfo => {
                let result = client.my_info();
                NetworkEvent::MyInfo(result)
            }
        }
    }
}
