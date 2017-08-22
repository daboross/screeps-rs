use std::time::Duration;
use std::rc::Rc;
use std::cell::{Ref, RefCell};
use std::ops::Deref;
use std::sync::Arc;

use std::sync::mpsc::Sender as StdSender;
use futures::sync::mpsc::Sender as BoundedFuturesSender;

use futures::{future, Future, Sink};
use tokio_core::reactor::{Handle, Timeout};
use hyper::StatusCode;

use screeps_api::{self, TokenStorage};

use hyper;

use event::NetworkEvent;

use diskcache;
use {ConnectionSettings, Notify};

use super::types::HttpRequest;
use super::utils;

pub struct Executor<N, C, H, T> {
    pub handle: Handle,
    pub send_results: StdSender<NetworkEvent>,
    pub notify: N,
    pub executor_return: BoundedFuturesSender<Executor<N, C, H, T>>,
    pub settings: Rc<RefCell<Arc<ConnectionSettings>>>,
    pub client: screeps_api::Api<C, H, T>,
    pub disk_cache: diskcache::Cache,
}

impl<'a, N, C, H, T> utils::HasClient<'a, C, H, T> for Executor<N, C, H, T>
where
    C: hyper::client::Connect,
    H: screeps_api::HyperClient<C>,
    T: TokenStorage,
{
    type SettingsDeref = Ref<'a, ConnectionSettings>;

    fn settings(&'a self) -> Self::SettingsDeref {
        Ref::map(self.settings.borrow(), Deref::deref)
    }
    fn api(&'a self) -> &'a screeps_api::Api<C, H, T> {
        &self.client
    }
}

enum HttpExecError<N, C, H, T> {
    Continue(Executor<N, C, H, T>),
    Exit,
}

impl<N, C, H, T> Executor<N, C, H, T>
where
    C: hyper::client::Connect,
    H: screeps_api::HyperClient<C> + 'static + Clone,
    T: TokenStorage,
    N: Notify,
{
    fn exec_network(
        self,
        request: HttpRequest,
    ) -> Box<Future<Item = (Self, HttpRequest, NetworkEvent), Error = HttpExecError<N, C, H, T>> + 'static> {
        match request {
            HttpRequest::Login => {
                let username;
                Box::new(
                    {
                        let settings = self.settings.borrow();
                        username = settings.username.to_owned();
                        self.client.login(&*settings.username, &*settings.password)
                    }.then(move |result| {
                        let event = NetworkEvent::Login {
                            username: username,
                            result: result.map(|logged_in| logged_in.return_to(&self.client.tokens)),
                        };

                        future::ok((self, HttpRequest::Login, event))
                    }),
                )
            }
            HttpRequest::MyInfo => {
                let execute = |executor: Self| match executor.client.my_info() {
                    Ok(future) => Ok(future.then(move |result| {
                        future::ok((executor, HttpRequest::MyInfo, NetworkEvent::MyInfo { result: result }))
                    })),
                    Err(e) => Err((executor, e)),
                };

                let handle_err = |executor: Self, login_error| {
                    future::ok((
                        executor,
                        HttpRequest::MyInfo,
                        NetworkEvent::MyInfo {
                            result: Err(login_error),
                        },
                    ))
                };

                utils::execute_or_login_and_execute(self, execute, handle_err)
            }
            HttpRequest::ShardList => Box::new(self.client.shard_list().then(move |result| {
                future::ok((
                    self,
                    HttpRequest::ShardList,
                    NetworkEvent::ShardList {
                        result: match result {
                            Ok(v) => Ok(Some(v)),
                            Err(e) => match *e.kind() {
                                screeps_api::ErrorKind::StatusCode(hyper::StatusCode::NotFound) => Ok(None),
                                _ => Err(e),
                            },
                        },
                    },
                ))
            })),
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
                            Box::new(
                                self.client
                                    .room_terrain("shard0", room_name.to_string())
                                    .map(|data| data.terrain)
                                    .then(move |result| {
                                        if let Ok(ref data) = result {
                                            self.handle.spawn(
                                                self.disk_cache.set_terrain(room_name, data).then(|result| {
                                                    if let Err(e) = result {
                                                        warn!("error occurred storing to terrain cache: {}", e);
                                                    }
                                                    Ok(())
                                                }),
                                            );
                                        }
                                        future::ok((self, result))
                                    }),
                            ) as Box<Future<Item = _, Error = _>>
                        }
                    }.and_then(move |(executor, result)| {
                        future::ok((
                            executor,
                            HttpRequest::RoomTerrain {
                                room_name: room_name,
                            },
                            NetworkEvent::RoomTerrain {
                                room_name: room_name,
                                result: result,
                            },
                        ))
                    })
                }))
            }
            HttpRequest::ChangeSettings { settings } => {
                {
                    let mut current = self.settings.borrow_mut();
                    match (
                        settings.username == current.username,
                        settings.password == current.password,
                        settings.shard == current.shard,
                    ) {
                        (true, true, true) => (),
                        (true, false, _) | (true, _, false) => *current = settings.clone(),
                        (false, _, _) => {
                            *current = settings.clone();
                            while let Some(_) = self.client.tokens.take_token() {}
                        }
                    }
                }
                Box::new(future::err(HttpExecError::Continue(self)))
            }
            HttpRequest::Exit => Box::new(future::err(HttpExecError::Exit)),
        }
    }

    pub fn execute(self, request: HttpRequest) -> impl Future<Item = (), Error = ()> + 'static {
        self.exec_network(request)
            .then(move |result| -> Box<Future<Item = (), Error = ()> + 'static> {
                let exec = match result {
                    Ok((exec, request, event)) => {
                        if let Some(err) = event.error() {
                            if let screeps_api::ErrorKind::StatusCode(ref status) = *err.kind() {
                                if *status == StatusCode::TooManyRequests {
                                    debug!("starting 5-second timeout from TooManyRequests error.");

                                    let timeout = Timeout::new(Duration::from_secs(5), &exec.handle).expect(
                                        "expected Timeout::new() to only fail if tokio \
                                         core has been stopped",
                                    );

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
                                let result = exec.notify.wakeup();
                                if let Err(_) = result {
                                    warn!("failed to wake up main event loop after sending result successfully.")
                                }
                            }
                            Err(_) => {
                                warn!("failed to send the result of a request.");
                            }
                        }
                        exec
                    }
                    Err(HttpExecError::Continue(exec)) => exec,
                    Err(HttpExecError::Exit) => return Box::new(future::err(())),
                };

                Box::new(exec.executor_return.clone().send(exec).then(|result| {
                    if let Err(_) = result {
                        warn!("couldn't return connection token after finishing a request.")
                    };
                    future::ok(())
                }))
            })
    }
}
