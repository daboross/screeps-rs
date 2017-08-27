use std::sync::Arc;

use {glutin, screeps_rs_network};

#[derive(Clone)]
pub struct GlutinNotify(Arc<glutin::EventsLoopProxy>);

impl screeps_rs_network::Notify for GlutinNotify {
    fn wakeup(&self) -> Result<(), screeps_rs_network::Disconnected> {
        self.0
            .wakeup()
            .map_err(|glutin::EventsLoopClosed| screeps_rs_network::Disconnected)
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


pub type NetworkHandler = screeps_rs_network::TokioHandler<GlutinNotify>;
