use std::{fmt, thread};
use std::cell::RefCell;
use std::sync::Arc;
use std::rc::Rc;

use std::sync::mpsc as std_mpsc;
use std::sync::mpsc::Sender as StdSender;
use std::sync::mpsc::Receiver as StdReceiver;

use futures::sync::mpsc as futures_mpsc;
use futures::sync::mpsc::UnboundedSender as FuturesSender;
use futures::sync::mpsc::UnboundedReceiver as FuturesReceiver;

use futures::{future, Future, Stream};
use tokio_core::reactor::{Core, Remote};

use screeps_api::{self, ArcTokenStorage};

use {hyper, hyper_tls, tokio_core};

use event::NetworkEvent;
use request::Request;
use {ConnectionSettings, Notify, ScreepsConnection};
use diskcache;

mod types;
mod http;
mod ws;
mod utils;

use self::types::{GenericRequest, HttpRequest, WebsocketRequest};

pub struct Handler<N> {
    /// Receiver and sender interacting with the current threaded handler.
    ///
    /// Use std sync channel for (tokio -> main thread), and a futures channel for (main thread -> tokio):
    /// - neither have any specific requirements for where the sender is called, but both require that the
    ///   polling receiver be in the 'right context'. This way, it just works.
    handles: Option<HandlerHandles>,
    /// Tokens saved.
    tokens: ArcTokenStorage,
    /// Settings
    settings: Arc<ConnectionSettings>,
    /// Disk cache database Handle
    disk_cache: diskcache::Cache,
    /// Window proxy in case we need to restart handler thread.
    notify: N,
}

#[derive(Debug)]
struct HandlerHandles {
    remote: Remote,
    http_send: FuturesSender<HttpRequest>,
    ws_send: FuturesSender<WebsocketRequest>,
    recv: StdReceiver<NetworkEvent>,
}

impl HandlerHandles {
    fn new(
        remote: Remote,
        http_send: FuturesSender<HttpRequest>,
        ws_send: FuturesSender<WebsocketRequest>,
        recv: StdReceiver<NetworkEvent>,
    ) -> Self {
        HandlerHandles {
            remote: remote,
            http_send: http_send,
            ws_send: ws_send,
            recv: recv,
        }
    }

    fn send(&mut self, request: Request) -> Result<(), Request> {
        match request.into() {
            GenericRequest::Http(r) => self.http_send
                .unbounded_send(r)
                .map_err(|e| e.into_inner().into()),
            GenericRequest::Websocket(r) => self.ws_send
                .unbounded_send(r)
                .map_err(|e| e.into_inner().into()),
            GenericRequest::Both(hr, wr) => self.http_send
                .unbounded_send(hr)
                .map_err(|e| e.into_inner().into())
                .and_then(|()| {
                    self.ws_send
                        .unbounded_send(wr)
                        .map_err(|e| e.into_inner().into())
                }),
        }
    }
}

impl<N> Handler<N> {
    /// Creates a new handler, with the given settings and notify callback.
    pub fn new(settings: ConnectionSettings, notify: N) -> Self {
        Handler {
            settings: Arc::new(settings),
            handles: None,
            tokens: ArcTokenStorage::default(),
            // TODO: handle this gracefully
            disk_cache: diskcache::Cache::load().expect("loading the disk cache failed."),
            notify: notify,
        }
    }
}

impl<N: Notify> Handler<N> {
    fn start_handler(&mut self) {
        let mut queued: Option<Vec<NetworkEvent>> = None;
        if let Some(handles) = self.handles.take() {
            let mut queued_vec = Vec::new();
            while let Ok(v) = handles.recv.try_recv() {
                queued_vec.push(v);
            }
            queued = Some(queued_vec);
        }

        let (http_send_to_handler, handler_http_recv) = futures_mpsc::unbounded();
        let (ws_send_to_handler, handler_ws_recv) = futures_mpsc::unbounded();
        let (handler_send, recv_from_handler) = std_mpsc::channel();

        if let Some(values) = queued {
            for v in values {
                // fake these coming from the new handler.
                handler_send
                    .send(v)
                    .expect("expected handles to still be in current scope");
            }
        }

        let handler = ThreadedHandler::new(
            handler_http_recv,
            handler_ws_recv,
            handler_send,
            self.notify.clone(),
            self.tokens.clone(),
            self.settings.clone(),
            self.disk_cache.clone(),
        );

        let remote = handler.start_async_and_get_remote();

        self.handles = Some(HandlerHandles::new(
            remote,
            http_send_to_handler,
            ws_send_to_handler,
            recv_from_handler,
        ));
    }
}

impl<N: Notify> ScreepsConnection for Handler<N> {
    fn send(&mut self, request: Request) {
        // TODO: find out how to get panic info from the threaded thread, and report that we had to reconnect!
        let request_retry = match self.handles {
            Some(ref mut handles) => match handles.send(request) {
                Ok(()) => None,
                Err(request) => Some(request),
            },
            None => Some(request),
        };

        if let Some(request) = request_retry {
            self.start_handler();
            let send = self.handles
                .as_mut()
                .expect("expected handles to exist after freshly restarting");
            send.send(request)
                .expect("expected freshly started handler to still be running");
        }
    }

    fn poll(&mut self) -> Option<NetworkEvent> {
        let (evt, reset) = match self.handles {
            Some(ref mut handles) => match handles.recv.try_recv() {
                Ok(v) => (Some(v), false),
                Err(std_mpsc::TryRecvError::Empty) => (None, false),
                Err(std_mpsc::TryRecvError::Disconnected) => (None, true),
            },
            None => (None, false),
        };
        if reset {
            self.handles = None;
        }
        evt
    }
}

impl<N> fmt::Debug for Handler<N> {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.debug_struct("Handler")
            .field("handles", &self.handles)
            .field("settings", &self.settings)
            .field("tokens", &self.tokens)
            .field("notify", &"<non-debug>")
            .finish()
    }
}

struct ThreadedHandler<N> {
    http_recv: FuturesReceiver<HttpRequest>,
    ws_recv: FuturesReceiver<WebsocketRequest>,
    send: StdSender<NetworkEvent>,
    notify: N,
    settings: Arc<ConnectionSettings>,
    tokens: ArcTokenStorage,
    disk_cache: diskcache::Cache,
}
impl<N: Notify> ThreadedHandler<N> {
    fn new(
        http_recv: FuturesReceiver<HttpRequest>,
        ws_recv: FuturesReceiver<WebsocketRequest>,
        send: StdSender<NetworkEvent>,
        notify: N,
        tokens: ArcTokenStorage,
        settings: Arc<ConnectionSettings>,
        disk_cache: diskcache::Cache,
    ) -> Self {
        ThreadedHandler {
            http_recv: http_recv,
            ws_recv: ws_recv,
            send: send,
            notify: notify,
            settings: settings,
            tokens: tokens,
            disk_cache: disk_cache,
        }
    }

    fn start_async_and_get_remote(self) -> tokio_core::reactor::Remote {
        let (temp_sender, temp_receiver) = std_mpsc::channel();
        thread::spawn(|| self.run(temp_sender));
        temp_receiver
            .recv()
            .expect("expected newly created channel to not be dropped, perhaps tokio core panicked?")
    }

    fn run(self, send_remote_to: StdSender<tokio_core::reactor::Remote>) {
        use futures::Sink;

        let ThreadedHandler {
            mut http_recv,
            ws_recv,
            send,
            notify,
            settings,
            tokens,
            disk_cache,
        } = self;

        let settings_rc = Rc::new(RefCell::new(settings.clone()));

        let mut core = Core::new().expect("expected tokio core to succeed startup.");

        {
            // move into scope to drop.
            let sender = send_remote_to;
            sender
                .send(core.remote())
                .expect("expected sending remote to spawning thread to succeed.");
        }

        let handle = core.handle();

        disk_cache
            .start_cache_clean_task(&handle)
            .expect("expected starting database cleanup interval to succeed");

        let hyper = hyper::Client::configure()
            .connector(
                hyper_tls::HttpsConnector::new(4, &handle)
                    .expect("expected HTTPS handler construction with default parameters to succeed."),
            )
            .build(&handle);

        let mut client = screeps_api::Api::with_url_and_tokens(hyper, settings_rc.borrow().api_url.clone(), tokens)
            .expect("expected already parsed URL to parse as URL");

        struct StopAndClearPool(HttpRequest);

        let ws_executor = ws::Executor::new(
            handle.clone(),
            send.clone(),
            client.clone(),
            settings,
            notify.clone(),
        );

        // WS executor can just run in the background. Since there's only one
        // "executor", we don't need to restart it in the loop.
        handle.spawn(ws_executor.run(ws_recv));

        // Loop so that we can "flush" the pool of pending executions whenever
        // we're changing settings.
        loop {
            let (mut exec_pool_send, mut exec_pool_recv) = futures_mpsc::channel(5);

            // fill with 5 tokens.
            for _ in 0..5 {
                let cloned_send = exec_pool_send.clone();
                assert!(
                    exec_pool_send
                        .start_send(http::Executor {
                            handle: handle.clone(),
                            send_results: send.clone(),
                            notify: notify.clone(),
                            executor_return: cloned_send,
                            settings: settings_rc.clone(),
                            client: client.clone(),
                            disk_cache: disk_cache.clone(),
                        })
                        .expect("expected newly created channel to still be in scope")
                        .is_ready()
                );
            }

            // zip combines executors with requests so we'll
            // never be running more than 5 concurrent requests.
            let result = core.run(
                http_recv
                    .by_ref()
                    .zip(exec_pool_recv.by_ref())
                    .map_err(|()| panic!("expected futures::mpsc::sync::Receiver stream to never return an error."))
                    .for_each(|(request, executor)| {
                        if let HttpRequest::ChangeSettings { .. } = request {
                            exec_pool_send
                                .clone()
                                .start_send(executor)
                                .expect("expected channel to still be in scope");
                            future::err(StopAndClearPool(request))
                        } else {
                            // execute request returns the executor to the token pool at the end.
                            handle.spawn(executor.execute(request));
                            future::ok(())
                        }
                    }),
            );
            let last_request = match result {
                Ok(()) => break,
                Err(StopAndClearPool(request_to_do)) => request_to_do,
            };
            // HTTP executor receiving ChangeSettings will have already changed
            // the client's settings.
            //
            // Let's first just wait on all executors finishing their last requests, then process the
            // request we were waiting for, then restart the loop.
            core.run(exec_pool_recv.by_ref().take(4).for_each(|exec| {
                drop(exec);
                future::ok(())
            })).expect("expected futures::mpsc::sync::Receiver stream to never return an error.");
            core.run(
                exec_pool_recv
                    .by_ref()
                    .into_future()
                    .map_err(|((), _)| panic!("expected futures::mpsc::sync::Receiver to never return an error."))
                    .and_then(|(executor, _)| {
                        executor
                            .expect("expected pool to contain 5 executors")
                            .execute(last_request)
                    }),
            ).expect("expected Executor::execute to never return an errror");

            // and now, in case the client URL has changed, update it. This is necessary
            // since each cloned executor has a cloned URL, and cannot update the original.
            let settings = settings_rc.borrow();
            if settings.api_url != client.url {
                client.url = settings.api_url.clone();
            }
        }

        info!("single threaded event loop exiting.");
        // let the client know that we have closed, ignoring errors.
        let _ = notify.wakeup();
    }
}
