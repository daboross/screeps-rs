use std::default::Default;

use conrod::{self, color, Colorable, Labelable, Positionable, Sizeable, Widget, Borderable};
use conrod::widget::*;

use screeps_api;

use network;

use super::super::AppCell;
use super::{GraphicsState, PanelStates, frame, left_panel_available};

#[derive(Debug)]
pub struct RoomViewState {
    network: network::NetworkRequests,
    panels: PanelStates,
}

impl RoomViewState {
    pub fn new(network: network::NetworkRequests) -> Self {
        RoomViewState {
            network: network,
            panels: PanelStates::default(),
        }
    }
}

pub fn create_ui(app: &mut AppCell, state: &mut RoomViewState, update: &mut Option<GraphicsState>) {
    let AppCell { ref mut ui, ref mut net_cache, ref ids, .. } = *app;
    let body = Canvas::new()
        .color(color::DARK_CHARCOAL)
        .border(5.0)
        .border_color(color::DARK_GREY);
    frame(ui, ids, body);
    left_panel_available(ui, ids, &mut state.panels, update);

    {
        let mut net = net_cache.align(&mut state.network);
        if let Some(info) = net.my_info() {
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
}
