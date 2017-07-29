use std::sync::Arc;

pub use scrs_network::*;

use {glutin, scrs_network};

#[derive(Clone)]
pub struct GlutinNotify(Arc<glutin::EventsLoopProxy>);

impl scrs_network::Notify for GlutinNotify {
    fn wakeup(&self) -> Result<(), scrs_network::Disconnected> {
        self.0.wakeup().map_err(|glutin::EventsLoopClosed| scrs_network::Disconnected)
    }
}

impl From<Arc<glutin::EventsLoopProxy>> for GlutinNotify {
    fn from(arc: Arc<glutin::EventsLoopProxy>) -> Self {
        GlutinNotify(arc)
    }
}

impl From<glutin::EventsLoopProxy> for GlutinNotify {
    fn from(notify: glutin::EventsLoopProxy) -> Self {
        GlutinNotify(Arc::new(notify))
    }
}


pub type ThreadedHandler = scrs_network::TokioHandler<GlutinNotify>;
