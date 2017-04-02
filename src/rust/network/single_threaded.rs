use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::fmt;

use glutin;
use hyper;
use screeps_api;

use super::{Request, NetworkEvent, ScreepsConnection, NotLoggedIn};

pub struct Handler {
    /// Receiver and sender interacting with the current threaded handler.
    handles: Option<(Sender<Request<'static>>, Receiver<NetworkEvent>)>,
    /// Username and password in case we need to re-login.
    login_info: Option<(String, String)>,
    /// Window proxy in case we need to restart handler thread.
    window: glutin::WindowProxy,
}

impl Handler {
    /// Creates a new requests state, and starts an initial handler with a pending login request.
    pub fn new(window: glutin::WindowProxy) -> Self {
        Handler {
            handles: None,
            login_info: None,
            window: window,
        }
    }

    fn start_handler(&mut self) -> Result<(), NotLoggedIn> {
        let (username, password) = match self.login_info {
            Some(ref tuple) => tuple.clone(),
            None => return Err(NotLoggedIn),
        };

        let mut queued: Option<Vec<NetworkEvent>> = None;
        if let Some((_send, recv)) = self.handles.take() {
            let mut queued_vec = Vec::new();
            while let Ok(v) = recv.try_recv() {
                queued_vec.push(v);
            }
            queued = Some(queued_vec);
        }

        let (send_to_handler, handler_recv) = mpsc::channel();
        let (handler_send, recv_from_handler) = mpsc::channel();

        send_to_handler.send(Request::login(username, password))
            .expect("expected handles to still be in current scope");

        if let Some(values) = queued {
            for v in values {
                // fake these coming from the new handler.
                handler_send.send(v).expect("expected handles to still be in current scope");
            }
        }

        self.handles = Some((send_to_handler, recv_from_handler));

        let handler = ThreadedHandler::new(handler_recv, handler_send, self.window.clone());
        thread::spawn(move || handler.run());

        Ok(())
    }
}

impl ScreepsConnection for Handler {
    fn send(&mut self, request: Request) -> Result<(), NotLoggedIn> {
        let request = request.into_static();

        // TODO: find out how to get panic info from the threaded thread, and report that we had to reconnect!
        let request_retry = match self.handles {
            Some((ref send, _)) => {
                match send.send(request) {
                    Ok(()) => None,
                    Err(mpsc::SendError(request)) => Some(request),
                }
            }
            None => Some(request),
        };

        if let Some(request) = request_retry {
            match request {
                Request::Login { username, password } => {
                    self.login_info = Some((username.into_owned(), password.into_owned()));
                    self.start_handler()?;
                }
                request => {
                    self.start_handler()?;
                    let send = &self.handles.as_ref().expect("expected handles to exist after freshly restarting").0;
                    send.send(request).expect("expected freshly started handler to still be running");
                }
            }
        }

        Ok(())
    }

    fn poll(&mut self) -> Option<NetworkEvent> {
        let (evt, reset) = match self.handles {
            Some((_, ref mut recv)) => {
                match recv.try_recv() {
                    Ok(v) => (Some(v), false),
                    Err(mpsc::TryRecvError::Empty) => (None, false),
                    Err(mpsc::TryRecvError::Disconnected) => (None, true),
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

// custom debug which does not leak username and password.
impl fmt::Debug for Handler {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.debug_struct("Handler")
            .field("handles", &self.handles)
            .field("login_info",
                   &self.login_info.as_ref().map(|&(ref user, _)| (user, "<redacted>")))
            .field("window", &"<non-debug>")
            .finish()
    }
}

struct ThreadedHandler {
    client: screeps_api::API<hyper::Client>,
    recv: Receiver<Request<'static>>,
    send: Sender<NetworkEvent>,
    window: glutin::WindowProxy,
}

impl ThreadedHandler {
    fn new(recv: Receiver<Request<'static>>, send: Sender<NetworkEvent>, awaken: glutin::WindowProxy) -> Self {
        let hyper_client =
            hyper::Client::with_connector(hyper::net::HttpsConnector::new(::hyper_rustls::TlsClient::new()));
        ThreadedHandler {
            client: screeps_api::API::new(hyper_client),
            recv: recv,
            send: send,
            window: awaken,
        }
    }

    fn run(self) {
        let ThreadedHandler { mut client, recv, send, window } = self;
        loop {
            match recv.recv() {
                Ok(request) => {
                    let result = request.exec_with(&mut client);
                    debug!("successfully executed a request.");
                    match send.send(result) {
                        Ok(()) => (),
                        Err(mpsc::SendError(_)) => break,
                    };
                    window.wakeup_event_loop();
                }
                Err(mpsc::RecvError) => break,
            }
        }
        info!("single threaded event loop exiting.");
        // let the client know that we have closed.
        window.wakeup_event_loop();
    }
}
