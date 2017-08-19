use std::sync::Arc;

pub use screeps_rs_network::*;

use glutin::{self, EventsLoopProxy};

#[derive(Clone)]
pub struct GlutinNotify(Arc<EventsLoopProxy>);

impl Notify for GlutinNotify {
    fn wakeup(&self) -> Result<(), Disconnected> {
        self.0
            .wakeup()
            .map_err(|glutin::EventsLoopClosed| Disconnected)
    }
}

impl From<Arc<EventsLoopProxy>> for GlutinNotify {
    fn from(arc: Arc<EventsLoopProxy>) -> Self {
        GlutinNotify(arc)
    }
}

impl From<EventsLoopProxy> for GlutinNotify {
    fn from(notify: EventsLoopProxy) -> Self {
        GlutinNotify(Arc::new(notify))
    }
}


pub type ThreadedHandler = TokioHandler<GlutinNotify>;
