use std::collections::HashSet;
use std::rc::Rc;
use std::cell::{Cell, RefCell};
use std::sync::Arc;

use std::sync::mpsc::Sender as StdSender;
use futures::sync::mpsc as futures_mpsc;
use futures::sync::mpsc::UnboundedSender as FuturesSender;
use futures::sync::mpsc::UnboundedReceiver as FuturesReceiver;

use futures::{future, stream, Future, Sink, Stream};
use tokio_core::reactor::Handle;

use screeps_api::{self, NoToken, RoomName, TokenStorage};
use screeps_api::websocket::Channel;

use {hyper, websocket};

use event::NetworkEvent;
use {ConnectionSettings, Notify};

use super::types::WebsocketRequest;
use super::utils;

mod types {
    use futures::stream::{SplitSink, SplitStream};
    use websocket::client::async::{Framed, TcpStream, TlsStream};
    use websocket::codec::ws::MessageCodec;
    use websocket::OwnedMessage;

    pub type WebsocketMergedStream = Framed<TlsStream<TcpStream>, MessageCodec<OwnedMessage>>;
    pub type WebsocketSink = SplitSink<Framed<TlsStream<TcpStream>, MessageCodec<OwnedMessage>>>;
    pub type WebsocketStream = SplitStream<Framed<TlsStream<TcpStream>, MessageCodec<OwnedMessage>>>;
}

use self::types::{WebsocketMergedStream, WebsocketSink};
use self::read::ReaderData;

pub struct Executor<N, C, H, T> {
    handle: Handle,
    send_results: StdSender<NetworkEvent>,
    notify: N,
    http_client: screeps_api::Api<C, H, T>,
    settings: Arc<ConnectionSettings>,
    /// Receive messages from the Reader thread to send.
    raw_send_receiver: Option<FuturesReceiver<(u16, websocket::OwnedMessage)>>,
    raw_send_sender: FuturesSender<(u16, websocket::OwnedMessage)>,
    /// Unique connection ID so that raw messages meant to be sent to an old connection
    /// can be ignored/dropped.
    connection_id: u16,
    client: Option<WebsocketSink>,
    // What we're currently subscribed to:
    subscribed_map_view: Rc<RefCell<HashSet<RoomName>>>,
    subscribed_room_view: Rc<Cell<Option<RoomName>>>,
}

impl<N, C, H, T> Executor<N, C, H, T> {
    pub fn new(
        handle: Handle,
        send_results: StdSender<NetworkEvent>,
        http_client: screeps_api::Api<C, H, T>,
        settings: Arc<ConnectionSettings>,
        notify: N,
    ) -> Self {
        let (raw_sender, raw_receiver) = futures_mpsc::unbounded();

        Executor {
            handle: handle,
            send_results: send_results,
            notify: notify,
            http_client: http_client,
            settings: settings,
            raw_send_receiver: Some(raw_receiver),
            raw_send_sender: raw_sender,
            connection_id: 0,
            client: None,
            subscribed_map_view: Default::default(),
            subscribed_room_view: Default::default(),
        }
    }
}

impl<'a, N, C, H, T> utils::HasClient<'a, C, H, T> for Executor<N, C, H, T>
where
    C: hyper::client::Connect + 'static,
    H: screeps_api::HyperClient<C> + 'static,
    T: TokenStorage + 'static,
{
    type SettingsDeref = &'a ConnectionSettings;

    fn api(&'a self) -> &'a screeps_api::Api<C, H, T> {
        &self.http_client
    }

    fn settings(&'a self) -> Self::SettingsDeref {
        &self.settings
    }
}

enum WebsocketRequestOrRaw {
    Structured(WebsocketRequest),
    Raw(u16, websocket::OwnedMessage),
}

impl<N, C, H, T> Executor<N, C, H, T>
where
    C: hyper::client::Connect + 'static,
    H: screeps_api::HyperClient<C> + 'static,
    T: TokenStorage + 'static,
    N: Notify + 'static,
{
    pub fn run(mut self, ws_recv: FuturesReceiver<WebsocketRequest>) -> impl Future<Item = (), Error = ()> + 'static {
        ws_recv
            .map(|m| WebsocketRequestOrRaw::Structured(m))
            .select(
                self.raw_send_receiver
                    .take()
                    .expect("expected run to only ever be called once")
                    .map(|(id, m)| WebsocketRequestOrRaw::Raw(id, m)),
            )
            .fold(self, |executor, request| match request {
                WebsocketRequestOrRaw::Structured(request) => {
                    Box::new(executor.execute(request)) as Box<Future<Item = _, Error = _>>
                }
                WebsocketRequestOrRaw::Raw(id, message) => executor.send_raw(id, message),
            })
            .and_then(|executor| future::ok(drop(executor)))
    }


    fn execute(self, request: WebsocketRequest) -> impl Future<Item = Self, Error = ()> + 'static {
        match request {
            WebsocketRequest::SetMapSubscribes { rooms } => {
                let room_set = self.subscribed_map_view.clone();

                // subscribe to rooms we aren't subscribed to already.
                Box::new(
                    stream::iter(
                        rooms
                            .into_iter()
                            .filter(move |room_name| !room_set.borrow().contains(&room_name))
                            .map(|v| Ok(v)),
                    ).fold(self, |executor, room_name| {
                        let shard = executor.settings.shard.clone();
                        executor.subscribe(Channel::room_map_view(room_name, shard))
                    })
                        .and_then(move |executor| {
                            // and unsubscribe from rooms we no longer need data for.
                            let unneeded_rooms = executor
                                .subscribed_map_view
                                .borrow()
                                .iter()
                                .cloned()
                                .filter(|room_name| !rooms.contains(room_name))
                                .collect::<Vec<RoomName>>();

                            stream::iter(unneeded_rooms.into_iter().map(|v| Ok(v))).fold(
                                executor,
                                |executor, room_name| {
                                    let shard = executor.settings.shard.clone();
                                    executor.unsubscribe(Channel::room_map_view(room_name, shard))
                                },
                            )
                        }),
                ) as Box<Future<Item = Self, Error = ()>>
            }
            WebsocketRequest::SetFocusRoom { room } => match (self.subscribed_room_view.get(), room) {
                (None, None) => Box::new(future::ok(self)) as Box<Future<Item = Self, Error = ()>>,
                (Some(ref r1), Some(ref r2)) if r1 == r2 => Box::new(future::ok(self)),
                (None, Some(to_subscribe)) => {
                    let shard = self.settings.shard.clone();
                    Box::new(self.subscribe(Channel::room_detail(to_subscribe, shard)))
                }
                (Some(to_unsubscribe), None) => {
                    let shard = self.settings.shard.clone();
                    Box::new(self.unsubscribe(Channel::room_detail(to_unsubscribe, shard)))
                }
                (Some(to_unsubscribe), Some(to_subscribe)) => {
                    let shard = self.settings.shard.clone();
                    Box::new(
                        self.unsubscribe(Channel::room_detail(to_unsubscribe, shard))
                            .and_then(move |executor| {
                                let shard = executor.settings.shard.clone();
                                executor.subscribe(Channel::room_detail(to_subscribe, shard))
                            }),
                    )
                }
            },
            WebsocketRequest::ChangeSettings { settings } => unimplemented!(),
            WebsocketRequest::Exit => unimplemented!(),
        }
    }

    fn send_raw(mut self, id: u16, message: websocket::OwnedMessage) -> Box<Future<Item = Self, Error = ()>> {
        // ignore messages from past closed connections.
        if id == self.connection_id {
            if let Some(conn) = self.client.take() {
                return Box::new(self.send_into(conn, message).or_else(|err| Ok(err)));
            }
        }

        Box::new(future::ok(self))
    }

    fn relay_error(&self, error: websocket::WebSocketError) {
        self.relay_event(NetworkEvent::WebsocketError { error: error })
    }

    fn relay_http_error(&self, error: screeps_api::Error) {
        self.relay_event(NetworkEvent::WebsocketHttpError { error: error })
    }

    fn relay_event(&self, event: NetworkEvent) {
        match self.send_results.send(event) {
            Ok(()) => self.notify
                .wakeup()
                .expect("expected glium loop to still be running when "),
            Err(event) => {
                warn!("failed to send websocket event to main thread - event: {}", event);
            }
        };
    }

    fn send_into(
        mut self,
        sink: WebsocketSink,
        message: websocket::OwnedMessage,
    ) -> impl Future<Item = Self, Error = Self> + 'static {
        sink.send(message).then(move |result| match result {
            Ok(sink) => {
                self.client = Some(sink);

                future::ok(self)
            }
            Err(e) => {
                self.relay_error(e);

                future::err(self)
            }
        })
    }

    /// Error case can happen, and means the message failed to send.
    fn send(mut self, message: websocket::OwnedMessage) -> impl Future<Item = Self, Error = Self> + 'static {
        match self.client.take() {
            Some(sink) => Box::new(self.send_into(sink, message)) as Box<Future<Item = _, Error = _>>,
            None => {
                let login_failed = |executor: Self, err| {
                    executor.relay_http_error(err);

                    future::err(executor)
                };

                let get_token = |executor: Self| match executor.http_client.tokens.take_token() {
                    Some(t) => Ok(future::ok((executor, t))),
                    None => Err((executor, NoToken)),
                };

                // OK, first let's get a token to authenticate with:
                Box::new(
                    utils::execute_or_login_and_execute(self, get_token, login_failed).and_then(|(executor, token)| {
                        // Now actually start the websocket connection

                        let url = screeps_api::websocket::default_url();

                        let connection_future =
                            websocket::ClientBuilder::from_url(&url).async_connect_secure(None, &executor.handle);

                        connection_future.then(|result| match result {
                            Ok((connection, _)) => Box::new(
                                executor
                                    .login_protocol(connection, token)
                                    .and_then(|executor| executor.send(message)),
                            ) as Box<Future<Item = _, Error = _>>,
                            Err(e) => {
                                executor.relay_error(e);

                                Box::new(future::err(executor)) as Box<Future<Item = _, Error = _>>
                            }
                        })
                    }),
                ) as Box<Future<Item = _, Error = _>>
            }
        }
    }

    fn login_protocol(
        self,
        connection: WebsocketMergedStream,
        token: screeps_api::Token,
    ) -> impl Future<Item = Self, Error = Self> + 'static {
        let auth = websocket::OwnedMessage::Text(screeps_api::websocket::authenticate(&token));

        connection
            .send(auth)
            .then(|result| match result {
                Ok(connection) => {
                    fn finish_ping<N, C, H, T>(
                        executor: Executor<N, C, H, T>,
                        connection: WebsocketMergedStream,
                        data: Vec<u8>,
                    ) -> impl Future<
                        Item = (Executor<N, C, H, T>, WebsocketMergedStream),
                        Error = Executor<N, C, H, T>,
                    >
                                                 + 'static
                    where
                        C: hyper::client::Connect + 'static,
                        H: screeps_api::HyperClient<C> + 'static,
                        T: TokenStorage + 'static,
                        N: Notify + 'static,
                    {
                        connection
                            .send(websocket::OwnedMessage::Pong(data))
                            .then(|result| match result {
                                Ok(connection) => {
                                    Box::new(test_response(executor, connection)) as Box<Future<Item = _, Error = _>>
                                }
                                Err(e) => {
                                    executor.relay_error(e);

                                    Box::new(future::err(executor)) as Box<Future<Item = _, Error = _>>
                                }
                            })
                    }

                    fn test_response<N, C, H, T>(
                        executor: Executor<N, C, H, T>,
                        connection: WebsocketMergedStream,
                    ) -> impl Future<
                        Item = (Executor<N, C, H, T>, WebsocketMergedStream),
                        Error = Executor<N, C, H, T>,
                    >
                                                 + 'static
                    where
                        C: hyper::client::Connect + 'static,
                        H: screeps_api::HyperClient<C> + 'static,
                        T: TokenStorage + 'static,
                        N: Notify + 'static,
                    {
                        connection.into_future().then(|result| match result {
                            Ok((Some(message), connection)) => {
                                use screeps_api::websocket::parsing;

                                let text = match message {
                                    websocket::OwnedMessage::Text(text) => Ok(text),
                                    websocket::OwnedMessage::Ping(data) => {
                                        return Box::new(finish_ping(executor, connection, data)) as
                                            Box<Future<Item = _, Error = _>>
                                    }
                                    other => Err(parsing::ParseError::Other(
                                        format!("expected text websocket message, found {:?}", other),
                                    )),
                                };

                                let text = match text {
                                    Ok(v) => v,
                                    Err(e) => {
                                        executor.relay_event(NetworkEvent::WebsocketParseError { error: e });

                                        return Box::new(future::err(executor)) as Box<Future<Item = _, Error = _>>;
                                    }
                                };

                                let parsed = match parsing::SockjsMessage::parse(&text) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        executor.relay_event(NetworkEvent::WebsocketParseError { error: e });

                                        return Box::new(future::err(executor)) as Box<Future<Item = _, Error = _>>;
                                    }
                                };

                                match parsed {
                                    parsing::SockjsMessage::Message(m) => match m {
                                        parsing::ScreepsMessage::AuthOk { new_token } => {
                                            executor.http_client.tokens.return_token(new_token);

                                            Box::new(future::ok((executor, connection))) as
                                                Box<Future<Item = _, Error = _>>
                                        }
                                        parsing::ScreepsMessage::AuthFailed => {
                                            executor.relay_http_error(screeps_api::ErrorKind::Unauthorized.into());

                                            Box::new(future::err(executor)) as Box<Future<Item = _, Error = _>>
                                        }
                                        other => {
                                            warn!(
                                                "received unexpected websocket message while \
                                                 waiting for 'auth ok' response: {:?}",
                                                other
                                            );

                                            // Recursion here!
                                            Box::new(test_response(executor, connection)) as
                                                Box<Future<Item = _, Error = _>>
                                        }
                                    },
                                    parsing::SockjsMessage::Messages(m_list) => {
                                        for m in m_list {
                                            match m {
                                                parsing::ScreepsMessage::AuthOk { new_token } => {
                                                    executor.http_client.tokens.return_token(new_token);

                                                    return Box::new(future::ok((executor, connection))) as
                                                        Box<Future<Item = _, Error = _>>;
                                                }
                                                parsing::ScreepsMessage::AuthFailed => {
                                                    executor
                                                        .relay_http_error(screeps_api::ErrorKind::Unauthorized.into());

                                                    return Box::new(future::err(executor)) as
                                                        Box<Future<Item = _, Error = _>>;
                                                }
                                                other => {
                                                    warn!(
                                                        "received unexpected websocket message while \
                                                         waiting for 'auth ok' response: {:?}",
                                                        other
                                                    );
                                                }
                                            }
                                        }

                                        // Recursion here!
                                        Box::new(test_response(executor, connection)) as
                                            Box<Future<Item = _, Error = _>>
                                    }
                                    other => {
                                        warn!(
                                            "received unexpected websocket message while \
                                             waiting for 'auth ok' response: {:?}",
                                            other
                                        );

                                        // Recursion here!
                                        Box::new(test_response(executor, connection)) as
                                            Box<Future<Item = _, Error = _>>
                                    }
                                }
                            }
                            Ok((None, _)) => Box::new(future::err(executor)) as Box<Future<Item = _, Error = _>>,
                            Err((e, _)) => {
                                executor.relay_error(e);

                                Box::new(future::err(executor)) as Box<Future<Item = _, Error = _>>
                            }
                        })
                    }

                    Box::new(test_response(self, connection)) as Box<Future<Item = _, Error = _>>
                }
                Err(e) => {
                    self.relay_error(e);

                    Box::new(future::err(self)) as Box<Future<Item = _, Error = _>>
                }
            })
            .and_then(|(mut executor, connection): (Self, WebsocketMergedStream)| {
                executor.connection_id += 1;

                let (sink, stream) = connection.split();

                ReaderData::new(
                    executor.handle.clone(),
                    executor.send_results.clone(),
                    executor.http_client.tokens.clone(),
                    executor.notify.clone(),
                    executor.raw_send_sender.clone(),
                    executor.connection_id,
                ).start(stream);

                executor.client = Some(sink);

                future::ok(executor)
            })
    }

    fn subscribe(self, channel: Channel<'static>) -> impl Future<Item = Self, Error = ()> + 'static {
        let message = websocket::OwnedMessage::Text(screeps_api::websocket::subscribe(&channel));
        self.send(message)
            .and_then(move |executor| {
                match channel {
                    Channel::RoomMapView { room_name, .. } => {
                        executor.subscribed_map_view.borrow_mut().insert(room_name);
                    }
                    Channel::RoomDetail { room_name, .. } => {
                        executor.subscribed_room_view.set(Some(room_name));
                    }
                    other => {
                        warn!("websocket executor not prepared to handle registering channel {}", other);
                    }
                };
                Ok(executor)
            })
            .then(move |result| match result {
                Ok(executor) => future::ok(executor),
                Err(executor) => future::ok(executor),
            })
    }

    fn unsubscribe(self, channel: Channel<'static>) -> impl Future<Item = Self, Error = ()> + 'static {
        let message = websocket::OwnedMessage::Text(screeps_api::websocket::unsubscribe(&channel));
        self.send(message)
            .and_then(move |executor| {
                match channel {
                    Channel::RoomMapView {
                        room_name,
                        shard_name, // we just assume that there's only one shard for now.
                    } => {
                        executor.subscribed_map_view.borrow_mut().remove(&room_name);
                    }
                    Channel::RoomDetail {
                        room_name,
                        shard_name, // we just assume that there's only one shard for now.
                    } => if Some(room_name) == executor.subscribed_room_view.get() {
                        executor.subscribed_room_view.set(None);
                    },
                    other => {
                        warn!("websocket executor not prepared to handle registering channel {}", other);
                    }
                };
                Ok(executor)
            })
            .then(move |result| match result {
                Ok(executor) => future::ok(executor),
                Err(executor) => future::ok(executor),
            })
    }
}

mod read {
    use std::sync::mpsc::Sender as StdSender;

    use futures::{future, Future, Stream};
    use tokio_core::reactor::Handle;
    use websocket::{OwnedMessage, WebSocketError};
    use futures::sync::mpsc::UnboundedSender;

    use screeps_api::{self, TokenStorage};
    use screeps_api::websocket::{ChannelUpdate, ScreepsMessage, SockjsMessage};

    use event::NetworkEvent;
    use Notify;
    use super::types::WebsocketStream;

    pub struct ReaderData<N, T> {
        handle: Handle,
        send_results: StdSender<NetworkEvent>,
        tokens: T,
        notify: N,
        raw_send_sender: UnboundedSender<(u16, OwnedMessage)>,
        connection_id: u16,
    }

    /// marker error return to mean exiting the thread now.
    #[derive(Debug)]
    struct ExitNow;

    impl<N: Notify, T: TokenStorage> ReaderData<N, T> {
        pub fn new(
            handle: Handle,
            send_results: StdSender<NetworkEvent>,
            tokens: T,
            notify: N,
            send: UnboundedSender<(u16, OwnedMessage)>,
            connection_id: u16,
        ) -> Self {
            ReaderData {
                handle: handle,
                send_results: send_results,
                tokens: tokens,
                notify: notify,
                raw_send_sender: send,
                connection_id: connection_id,
            }
        }

        fn send(&self, event: NetworkEvent) -> Result<(), ExitNow> {
            match self.send_results.send(event) {
                Ok(()) => self.notify.wakeup().map_err(|_| ExitNow),
                Err(_) => {
                    debug!("sending websocket event to main thread failed, exiting.");
                    Err(ExitNow)
                }
            }
        }

        fn send_response(&self, response: OwnedMessage) -> Result<(), ExitNow> {
            UnboundedSender::send(&self.raw_send_sender, (self.connection_id, response)).map_err(|_| ExitNow)?;

            Ok(())
        }

        /// Consumes this ReaderData and the stream, and will read from the stream until it stops.
        ///
        /// This uses the stored Handle inside of ReaderData to put this task into the tokio core.
        pub fn start(self, stream: WebsocketStream) {
            self.handle.clone().spawn(
                stream
                    .then(|result| future::ok::<_, ExitNow>(result))
                    .fold(self, |executor, message| {
                        executor.event(message)?;
                        Ok(executor)
                    })
                    .then(|_| future::ok::<(), ()>(())),
            );
        }

        fn event(&self, message: Result<OwnedMessage, WebSocketError>) -> Result<(), ExitNow> {
            match message {
                Ok(m) => self.event_websocket_message(m),
                Err(e) => {
                    self.send(NetworkEvent::WebsocketError { error: e })?;

                    Ok(())
                }
            }
        }

        fn event_websocket_message(&self, message: OwnedMessage) -> Result<(), ExitNow> {
            match message {
                OwnedMessage::Text(text) => match SockjsMessage::parse(&text) {
                    Ok(message) => {
                        self.event_sockjs_message(message)?;
                    }
                    Err(e) => {
                        self.send(NetworkEvent::WebsocketParseError { error: e })?;
                    }
                },
                OwnedMessage::Binary(data) => {
                    warn!("ignoring binary data received on websocket: {:?}", data);
                }
                OwnedMessage::Close(reason) => {
                    warn!("websocket closed: {:?}", reason);
                }
                OwnedMessage::Ping(data) => {
                    self.send_response(OwnedMessage::Pong(data))?;
                }
                OwnedMessage::Pong(data) => {
                    // TODO: track how long the connection has been open and potentially close it
                    debug!("pong received: {:?}", data);
                }
            }

            Ok(())
        }
        fn event_sockjs_message(&self, message: SockjsMessage) -> Result<(), ExitNow> {
            match message {
                SockjsMessage::Open | SockjsMessage::Heartbeat => (),
                SockjsMessage::Close { code, reason } => {
                    warn!("sockjs connectoin closed ({}): {}", code, reason);
                }
                SockjsMessage::Message(message) => {
                    self.event_screeps_message(message)?;
                }
                SockjsMessage::Messages(messages) => for message in messages {
                    self.event_screeps_message(message)?;
                },
            }
            Ok(())
        }
        fn event_screeps_message(&self, message: ScreepsMessage) -> Result<(), ExitNow> {
            match message {
                msg @ ScreepsMessage::ServerProtocol { .. } |
                msg @ ScreepsMessage::ServerPackage { .. } |
                msg @ ScreepsMessage::ServerTime { .. } => {
                    debug!("received protocol message: {:?}", msg);
                }
                ScreepsMessage::AuthFailed => {
                    warn!(
                        "received 'auth failed' message from inside main handler, \
                         which only operates after auth response has been received."
                    );
                    self.send(NetworkEvent::WebsocketHttpError {
                        error: screeps_api::ErrorKind::Unauthorized.into(),
                    })?;
                }
                ScreepsMessage::AuthOk { new_token } => {
                    self.tokens.return_token(new_token);
                }
                ScreepsMessage::ChannelUpdate { update } => {
                    self.event_channel_update(update)?;
                }
                ScreepsMessage::Other(unparsed) => {
                    warn!("received screeps message which did not match any known format!\n\t{}", unparsed);
                }
            }
            Ok(())
        }

        fn event_channel_update(&self, update: ChannelUpdate) -> Result<(), ExitNow> {
            // TODO: what to do here if shard name does not equal saved shard? discard?
            match update {
                ChannelUpdate::RoomMapView {
                    room_name, update, ..
                } => {
                    let event = NetworkEvent::MapView {
                        room_name: room_name,
                        result: update,
                    };
                    debug!("received map view update for {}!", room_name);
                    self.send(event)?;
                }
                ChannelUpdate::RoomDetail {
                    room_name, update, ..
                } => {
                    let event = NetworkEvent::RoomView {
                        room_name: room_name,
                        result: update,
                    };
                    debug!("received room view update for {}!", room_name);
                    self.send(event)?;
                }
                other => {
                    warn!("received unexpected channel update: {:#?}", other);
                }
            }
            Ok(())
        }
    }
}
