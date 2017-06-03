use std::{fmt, thread};

use std::sync::mpsc::{self as std_mpsc, Sender as StdSender, Receiver as StdReceiver};

use futures::sync::mpsc::{self as futures_mpsc, UnboundedSender as FuturesSender, UnboundedReceiver as FuturesReceiver};

use futures::{future, Future, Stream};

use tokio_core::reactor::{Remote, Core};

use screeps_api::ArcTokenStorage;

use {glutin, hyper, hyper_tls, tokio_core, screeps_api};

use super::{LoginDetails, Request, NetworkEvent, ScreepsConnection, NotLoggedIn};

pub struct Handler {
    /// Receiver and sender interacting with the current threaded handler.
    ///
    /// Use std sync channel for (tokio -> main thread), and a futures channel for (main thread -> tokio):
    /// - neither have any specific requirements for where the sender is called, but both require that the
    ///   polling receiver be in the 'right context'. This way, it just works.
    handles: Option<(Remote, FuturesSender<Request>, StdReceiver<NetworkEvent>)>,
    /// Tokens saved.
    tokens: ArcTokenStorage,
    /// Username and password in case we need to re-login.
    login_info: Option<LoginDetails>,
    /// Window proxy in case we need to restart handler thread.
    window: glutin::WindowProxy,
}

impl Handler {
    /// Creates a new requests state, and starts an initial handler with a pending login request.
    pub fn new(window: glutin::WindowProxy) -> Self {
        Handler {
            handles: None,
            login_info: None,
            tokens: ArcTokenStorage::default(),
            window: window,
        }
    }

    fn start_handler(&mut self) -> Result<(), NotLoggedIn> {
        let login_details = match self.login_info {
            Some(ref tuple) => tuple.clone(),
            None => return Err(NotLoggedIn),
        };

        let mut queued: Option<Vec<NetworkEvent>> = None;
        if let Some((_, _send, recv)) = self.handles.take() {
            let mut queued_vec = Vec::new();
            while let Ok(v) = recv.try_recv() {
                queued_vec.push(v);
            }
            queued = Some(queued_vec);
        }

        let (send_to_handler, handler_recv) = futures_mpsc::unbounded();
        let (handler_send, recv_from_handler) = std_mpsc::channel();

        if let Some(values) = queued {
            for v in values {
                // fake these coming from the new handler.
                handler_send.send(v).expect("expected handles to still be in current scope");
            }
        }

        let handler = ThreadedHandler::new(handler_recv,
                                           handler_send,
                                           self.window.clone(),
                                           self.tokens.clone(),
                                           login_details.clone());

        let remote = handler.start_async_and_get_remote();

        self.handles = Some((remote, send_to_handler, recv_from_handler));

        Ok(())
    }
}

impl ScreepsConnection for Handler {
    fn send(&mut self, request: Request) -> Result<(), NotLoggedIn> {
        // TODO: find out how to get panic info from the threaded thread, and report that we had to reconnect!
        let request_retry = match self.handles {
            Some((_, ref mut send, _)) => {
                match send.send(request) {
                    Ok(()) => None,
                    Err(send_err) => Some(send_err.into_inner()),
                }
            }
            None => Some(request),
        };

        if let Some(request) = request_retry {
            match request {
                Request::Login { details } => {
                    self.login_info = Some(details);
                    self.start_handler()?;
                }
                request => {
                    self.start_handler()?;
                    let send = &self.handles.as_ref().expect("expected handles to exist after freshly restarting").1;
                    send.send(request).expect("expected freshly started handler to still be running");
                }
            }
        }

        Ok(())
    }

    fn poll(&mut self) -> Option<NetworkEvent> {
        let (evt, reset) = match self.handles {
            Some((_, _, ref mut recv)) => {
                match recv.try_recv() {
                    Ok(v) => (Some(v), false),
                    Err(std_mpsc::TryRecvError::Empty) => (None, false),
                    Err(std_mpsc::TryRecvError::Disconnected) => (None, true),
                }
            }
            None => (None, false),
        };
        if reset {
            self.handles = None;
        }
        evt
    }
}
impl fmt::Debug for Handler {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.debug_struct("Handler")
            .field("handles", &self.handles)
            .field("login_info", &self.login_info)
            .field("tokens", &self.tokens)
            .field("window", &"<non-debug>")
            .finish()
    }
}


struct ThreadedHandler {
    recv: FuturesReceiver<Request>,
    send: StdSender<NetworkEvent>,
    window: glutin::WindowProxy,
    login: LoginDetails,
    tokens: ArcTokenStorage,
}

impl ThreadedHandler {
    fn new(recv: FuturesReceiver<Request>,
           send: StdSender<NetworkEvent>,
           awaken: glutin::WindowProxy,
           tokens: ArcTokenStorage,
           login: LoginDetails)
           -> Self {
        ThreadedHandler {
            recv: recv,
            send: send,
            window: awaken,
            login: login,
            tokens: tokens,
        }
    }

    fn start_async_and_get_remote(self) -> tokio_core::reactor::Remote {
        let (temp_sender, temp_receiver) = std_mpsc::channel();
        thread::spawn(|| self.run(temp_sender));
        temp_receiver.recv().expect("expected newly created channel to not be dropped, perhaps tokio core panicked?")
    }

    fn run(self, send_remote_to: StdSender<tokio_core::reactor::Remote>) {
        use futures::Sink;

        let ThreadedHandler { recv, send, window, login, tokens } = self;

        let mut core = Core::new().expect("expected tokio core to succeed startup.");

        {
            // move into scope to drop.
            let sender = send_remote_to;
            sender.send(core.remote()).expect("expected sending remote to spawning thread to succeed.");
        }

        let handle = core.handle();

        let hyper = hyper::Client::configure()
            .connector(hyper_tls::HttpsConnector::new(4, &handle))
            .build(&handle);

        let client = screeps_api::Api::with_tokens(hyper, tokens);

        // token pool so we only have at max 5 connections open at a time.
        let (mut token_pool_send, token_pool_recv) = futures_mpsc::channel(5);

        // fill with 5 tokens.
        for _ in 0..5 {
            assert!(token_pool_send.start_send(()).expect("expected newly created channel to be empty").is_ready());
        }

        // zip ensures that we have one token for each request! this way we'll
        // never have more than 5 concurrent requests.
        let result = core.run(recv.zip(token_pool_recv).and_then(|(request, ())| {
            let send_results = send.clone();
            let window_notify = window.clone();
            let send_tokens = token_pool_send.clone();
            let request_finish_future = request.exec_with(&login, &client).and_then(move |event| {
                match send_results.send(event) {
                    Ok(_) => {
                        trace!("successfully finished a request.");
                        window_notify.wakeup_event_loop();
                    }
                    Err(_) => {
                        warn!("failed to send the result of a request.");
                    }
                }

                future::ok(())
            }).then(|_| {
                // in the case of success or failure, let's add one more token so we can start
                // another request.
                send_tokens.send(()).then(|result| {
                    if let Err(_) = result {
                        warn!("couldn't return connection token after finishing a request.")
                    };
                    future::ok(())
                })
            });

            handle.spawn(request_finish_future);

            future::ok(())
        }).fold((), |(), _| future::ok(())));

        if let Err(()) = result {
            warn!("Unexpected error when running network core.");
        }

        info!("single threaded event loop exiting.");
        // let the client know that we have closed.
        window.wakeup_event_loop();
    }
}
