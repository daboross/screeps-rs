use std::borrow::Cow;

use hyper;
use futures::{future, Future};
use screeps_api::{self, NoToken};

use self::Request::*;

use super::LoginDetails;

#[derive(Clone, Debug, Hash)]
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
        Request::RoomTerrain { room_name: room_name }
    }

    pub fn exec_with<C, H, T>(self,
                              login: &LoginDetails,
                              client: &screeps_api::Api<C, H, T>)
                              -> Box<Future<Item = NetworkEvent, Error = ()> + 'static>
        where C: hyper::client::Connect,
              H: screeps_api::HyperClient<C> + Clone + 'static,
              T: screeps_api::TokenStorage
    {
        match self {
            Login { details } => {
                let tokens = client.tokens.clone();
                Box::new(client.login(details.username(), details.password())
                    .then(move |result| {
                        future::ok(NetworkEvent::Login {
                            username: details.username().to_owned(),
                            result: result.map(|logged_in| logged_in.return_to(&tokens)),
                        })
                    }))
            }
            MyInfo => {
                match client.my_info() {
                    Ok(future) => Box::new(future.then(|result| future::ok(NetworkEvent::MyInfo { result: result }))),
                    Err(NoToken) => {
                        let client = client.clone();
                        Box::new(client.login(login.username(), login.password())
                            .and_then(move |login_ok| {
                                login_ok.return_to(&client.tokens);

                                // TODO: something here to avoid a race condition!
                                client.my_info().expect("just returned token")
                            })
                            .then(|result| future::ok(NetworkEvent::MyInfo { result: result })))
                    }
                }
            }
            RoomTerrain { room_name } => {
                Box::new(client.room_terrain(room_name.to_string()).then(move |result| {
                    future::ok(NetworkEvent::RoomTerrain {
                        room_name: room_name,
                        result: result,
                    })
                }))
            }
        }
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
        result: Result<screeps_api::RoomTerrain, screeps_api::Error>,
    },
}
