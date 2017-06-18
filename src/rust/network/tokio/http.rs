use std::time::Duration;

use std::sync::mpsc::Sender as StdSender;
use futures::sync::mpsc::Sender as BoundedFuturesSender;

use futures::{future, Future, Sink};
use tokio_core::reactor::{Handle, Timeout};
use hyper::StatusCode;

use screeps_api::{self, TokenStorage};

use {glutin, hyper};

use network::{LoginDetails, NetworkEvent};
use network::cache::disk;

use super::types::HttpRequest;
use super::utils;

pub struct Executor<C, H, T> {
    pub handle: Handle,
    pub send_results: StdSender<NetworkEvent>,
    pub notify: glutin::WindowProxy,
    pub executor_return: BoundedFuturesSender<Executor<C, H, T>>,
    pub login: LoginDetails,
    pub client: screeps_api::Api<C, H, T>,
    pub disk_cache: disk::Cache,
}

impl<C, H, T> utils::HasClient<C, H, T> for Executor<C, H, T>
    where C: hyper::client::Connect,
          H: screeps_api::HyperClient<C>,
          T: TokenStorage
{
    fn login(&self) -> &LoginDetails {
        &self.login
    }
    fn api(&self) -> &screeps_api::Api<C, H, T> {
        &self.client
    }
}

impl<C, H, T> Executor<C, H, T>
    where C: hyper::client::Connect,
          H: screeps_api::HyperClient<C> + 'static + Clone,
          T: TokenStorage
{
    fn exec_network(self,
                    request: HttpRequest)
                    -> Box<Future<Item = (Self, HttpRequest, NetworkEvent), Error = ()> + 'static> {
        match request {
            HttpRequest::Login { details } => {
                Box::new(self.client
                    .login(details.username(), details.password())
                    .then(move |result| {
                        let event = NetworkEvent::Login {
                            username: details.username().to_owned(),
                            result: result.map(|logged_in| logged_in.return_to(&self.client.tokens)),
                        };

                        future::ok((self, HttpRequest::Login { details: details }, event))
                    }))
            }
            HttpRequest::MyInfo => {
                let execute = |executor: Self| match executor.client.my_info() {
                    Ok(future) => {
                        Ok(future.then(move |result| {
                            future::ok((executor, HttpRequest::MyInfo, NetworkEvent::MyInfo { result: result }))
                        }))
                    }
                    Err(e) => Err((executor, e)),
                };

                let handle_err = |executor: Self, login_error| {
                    future::ok((executor, HttpRequest::MyInfo, NetworkEvent::MyInfo { result: Err(login_error) }))
                };

                utils::execute_or_login_and_execute(self, execute, handle_err)
            }
            HttpRequest::RoomTerrain { room_name } => {
                Box::new(self.disk_cache.get_terrain(room_name).then(move |result| {
                    match result {
                            Ok(Some(terrain)) => {
                                Box::new(future::ok((self, Ok(terrain)))) as Box<Future<Item = _, Error = _>>
                            }
                            other => {
                                if let Err(e) = other {
                                    warn!("error occurred fetching terrain cache: {}", e);
                                }
                                Box::new(self.client
                                    .room_terrain(room_name.to_string())
                                    .map(|data| data.terrain)
                                    .then(move |result| {
                                        if let Ok(ref data) = result {
                                            self.handle.spawn(self.disk_cache
                                                .set_terrain(room_name, data)
                                                .then(|result| {
                                                    if let Err(e) = result {
                                                        warn!("error occurred storing to terrain cache: {}", e);
                                                    }
                                                    Ok(())
                                                }));
                                        }
                                        future::ok((self, result))
                                    })) as Box<Future<Item = _, Error = _>>
                            }
                        }
                        .and_then(move |(executor, result)| {
                            future::ok((executor,
                                        HttpRequest::RoomTerrain { room_name: room_name },
                                        NetworkEvent::RoomTerrain {
                                            room_name: room_name,
                                            result: result,
                                        }))
                        })
                }))
            }
        }
    }

    pub fn execute(self, request: HttpRequest) -> impl Future<Item = (), Error = ()> + 'static {
        self.exec_network(request)
            .and_then(move |(exec, request, event)| -> Box<Future<Item = (), Error = ()> + 'static> {

                if let Some(err) = event.error() {
                    if let screeps_api::ErrorKind::StatusCode(ref status) = *err.kind() {
                        if *status == StatusCode::TooManyRequests {
                            debug!("starting 5-second timeout from TooManyRequests error.");

                            let timeout = Timeout::new(Duration::from_secs(5), &exec.handle)
                                .expect("expected Timeout::new() to only fail if tokio \
                                            core has been stopped");

                            return Box::new(timeout.then(|_| {
                                debug!("5-second timeout finished.");

                                exec.execute(request)
                            }));
                        }
                    }
                }

                match exec.send_results.send(event) {
                    Ok(_) => {
                        trace!("successfully finished a request.");
                        exec.notify.wakeup_event_loop();
                    }
                    Err(_) => {
                        warn!("failed to send the result of a request.");
                    }
                }

                Box::new(exec.executor_return.clone().send(exec).then(|result| {
                    if let Err(_) = result {
                        warn!("couldn't return connection token after finishing a request.")
                    };
                    future::ok(())
                }))
            })
    }
}
