//! Handling network connections in a separate thread.
//!
//! This currently only supports a single threaded thread, but work may be done to allow multiple concurrent network
//! connections.

use glutin;
use hyper;
use screeps_api;

use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;


pub struct NetworkRequests {
    /// Receiver and sender interacting with the current threaded handler.
    handles: Option<(Sender<RequestInfo>, Receiver<FinishedRequest>)>,
    /// Username and password in case we need to re-login.
    login_info: (String, String),
    /// Window proxy in case we need to restart handler thread.
    window: glutin::WindowProxy,
}

impl NetworkRequests {
    /// Creates a new requests state, and starts an initial handler with a pending login request.
    pub fn new(window: glutin::WindowProxy, username: String, password: String) -> Self {
        let mut requests = NetworkRequests {
            handles: None,
            login_info: (username.clone(), password.clone()),
            window: window,
        };

        /// this will create a new thread and send a login request (TODO: do we want to keep the token in an Arc and
        /// have it instead of keeping login details in memory?)
        requests.start_handler();

        requests
    }

    fn start_handler(&mut self) {
        let mut queued: Option<Vec<FinishedRequest>> = None;
        if let Some((_send, recv)) = self.handles.take() {
            let mut queued_vec = Vec::new();
            while let Ok(v) = recv.try_recv() {
                queued_vec.push(v);
            }
            queued = Some(queued_vec);
        }

        let (send_to_handler, handler_recv) = mpsc::channel();
        let (handler_send, recv_from_handler) = mpsc::channel();

        send_to_handler.send(RequestInfo::login(self.login_info.0.clone(), self.login_info.1.clone()))
            .expect("expected handles to still be in current scope");

        if let Some(values) = queued {
            for v in values {
                // fake these coming from the new handler.
                handler_send.send(v).expect("expected handles to still be in current scope");
            }
        }

        self.handles = Some((send_to_handler, recv_from_handler));

        let handler = ThreadedHandler::new(handler_recv, handler_send, Some(self.window.clone()));
        thread::spawn(move || handler.run());
    }

    pub fn send(&mut self, request: RequestInfo) {
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
                RequestInfo::Login { username, password } => {
                    self.login_info = (username, password);
                    self.start_handler();
                }
                request => {
                    self.start_handler();
                    let send = &self.handles.as_ref().expect("expected handles to exist after freshly restarting").0;
                    send.send(request).expect("expected freshly started handler to still be running");
                }
            }
        }
    }

    /// Logs in, saving the given username and password.
    pub fn login_with(&mut self, username: String, password: String) {
        self.login_info = (username.clone(), password.clone());
        let request = RequestInfo::login(self.login_info.0.clone(), self.login_info.1.clone());
        self.send(request);
    }

    pub fn poll(&mut self) -> Option<FinishedRequest> {
        match self.handles {
            // we don't really care about disconnected handles here.
            Some((_, ref mut recv)) => recv.try_recv().ok(),
            None => None,
        }
    }
}

// custom debug which does not leak username and password.
impl ::std::fmt::Debug for NetworkRequests {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.debug_struct("NetworkRequests")
            .field("handles", &self.handles)
            .field("login_info", &"<redacted>")
            .field("window", &"<non-debug>")
            .finish()
    }
}

#[derive(Debug)]
pub enum FinishedRequest {
    Login {
        username_requested: String,
        result: screeps_api::Result<()>,
    },
}

#[derive(Clone, Debug)]
pub enum RequestInfo {
    Login { username: String, password: String },
}

impl RequestInfo {
    pub fn login(username: String, password: String) -> Self {
        RequestInfo::Login {
            username: username,
            password: password,
        }
    }
}

struct ThreadedHandler {
    client: screeps_api::API<hyper::Client>,
    recv: Receiver<RequestInfo>,
    send: Sender<FinishedRequest>,
    awaken: Option<glutin::WindowProxy>,
}

impl ThreadedHandler {
    fn new(recv: Receiver<RequestInfo>, send: Sender<FinishedRequest>, to_awaken: Option<glutin::WindowProxy>) -> Self {
        let hyper_client =
            hyper::Client::with_connector(hyper::net::HttpsConnector::new(::hyper_rustls::TlsClient::new()));
        ThreadedHandler {
            client: screeps_api::API::new(hyper_client),
            recv: recv,
            send: send,
            awaken: to_awaken,
        }
    }

    fn run(self) {
        let ThreadedHandler { mut client, recv, send, awaken } = self;
        loop {
            match recv.recv() {
                Ok(request) => {
                    let result = match request {
                        RequestInfo::Login { username, password } => {
                            let result = client.login(&*username, &*password);
                            FinishedRequest::Login {
                                username_requested: username,
                                result: result,
                            }
                        }
                    };
                    match send.send(result) {
                        Ok(()) => (),
                        Err(mpsc::SendError(_)) => break,
                    };
                    if let Some(ref window) = awaken {
                        window.wakeup_event_loop();
                    }
                }
                Err(mpsc::RecvError) => break,
            }
        }
        // let the client know that we have closed.
        if let Some(ref window) = awaken {
            window.wakeup_event_loop();
        }
    }
}
