// Network
extern crate screeps_api;
extern crate scrs_network;
// Logging
extern crate chrono;
extern crate fern;
#[macro_use]
extern crate log;

#[allow(unused_imports)]
use jni_c_header::{jint, JNIEnv, jclass, jlong, jstring, jdouble, jfloat, jboolean, jobject, jfieldID, jobjectArray,
                   jmethodID, jsize, jshort, jbyte};

#[derive(Debug, Copy, Clone)]
struct Notify;

impl scrs_network::Notify for Notify {
    fn wakeup(&self) -> Result<(), scrs_network::Disconnected> {
        Ok(())
    }
}

struct ScreepsConnection {
    cache: scrs_network::MemCache,
    network: scrs_network::TokioHandler<Notify>,
}

type AllignedConnection<'a> = scrs_network::memcache::NetworkedMemCache<'a,
                                                                        scrs_network::TokioHandler<Notify>,
                                                                        fn(scrs_network::ErrorEvent)>;

impl ScreepsConnection {
    /// TODO: Accept callbacks for "error" and for "wakeup".
    // right now the network<->UI communication is basically:
    // - Each draw, UI requests everything it needs
    // - Backend eventually fetches things, retries, etc.
    // - Backend sends a generic "wake up" call whenever anything new is in.
    pub fn new() -> Self {
        ScreepsConnection {
            cache: scrs_network::MemCache::new(),
            network: scrs_network::tokio::Handler::new(Notify),
        }
    }

    fn alligned(&mut self) -> AllignedConnection {
        self.cache.align(&mut self.network,
                         |error| warn!("error occurred: {}", error))
    }

    pub fn login(&mut self, username: &str, password: &str) {
        self.alligned()
            .login(username, password);
    }
}

foreigner_class!(class ScreepsConnection {
    self_type ScreepsConnection;
    constructor ScreepsConnection::new() -> ScreepsConnection;
    method ScreepsConnection::login(&self, _: &str, _: &str) -> ();
//    method ScreepsConnection::my_info(&self) -> Option<screeps_api::MyInfo>;
});
