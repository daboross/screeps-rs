use std::default::Default;

use conrod::{self, color, Colorable, Labelable, Positionable, Sizeable, Widget, Borderable};
use conrod::widget::*;

use screeps_api;

use network;

use super::super::AppCell;
use super::{GraphicsState, PanelStates, frame, left_panel_available};

#[derive(Debug)]
pub struct RoomViewState<T: network::ScreepsConnection = network::ThreadedHandler> {
    network: T,
    panels: PanelStates,
}

impl<T: network::ScreepsConnection> RoomViewState<T> {
    pub fn new(network: T) -> Self {
        RoomViewState {
            network: network,
            panels: PanelStates::default(),
        }
    }

    pub fn into_network(self) -> T {
        self.network
    }
}

pub fn create_ui(app: &mut AppCell,
                 state: &mut RoomViewState,
                 update: &mut Option<GraphicsState>)
                 -> Result<(), network::NotLoggedIn> {
    let AppCell { ref mut ui, ref mut net_cache, ref ids, .. } = *app;
    let body = Canvas::new()
        .color(color::BLACK)
        .border(0.0);
    frame(ui, ids, body);
    left_panel_available(ui, ids, &mut state.panels, update);

    {
        let mut net = net_cache.align(&mut state.network);
        if let Some(info) = net.my_info()? {
            Text::new(&format!("{} - GCL {}", info.username, screeps_api::gcl_calc(info.gcl_points)))
                // style
                .font_size(ui.theme.font_size_small)
                .right_justify()
                .no_line_wrap()
                // position
                .mid_right_with_margin_on(ids.header, 10.0)
                .set(ids.username_gcl_header, ui);
        }
    }

    Ok(())
}
