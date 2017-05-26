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
    let AppCell { ref mut ui, ref mut net_cache, ref mut ids, .. } = *app;

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

        if let Some(terrain) = net.room_terrain(screeps_api::RoomName::new("E0N0").unwrap())? {
            info!("found: {:?}", terrain);
            // let mut id_gen = ui.widget_id_generator();
            // if ids.rows.len() < 50 {
            //     ids.rows.resize(50, &mut id_gen);
            // }
            // if ids.tiles.len() < 2500 {
            //     ids.tiles.resize(2500, &mut id_gen);
            // }
            // let mut row_walk = ids.rows.walk();
            // let mut tile_walk = ids.tiles.walk();
            // let rows = terrain.iter()
            //     .enumerate()
            //     .map(|(y, row)| {
            //         let next_row_id = row_walk.next(&mut ids.rows, &mut id_gen);
            //         let columns = row.iter()
            //             .enumerate()
            //             .map(|(x, terrain)| {
            //                 use screeps_api::endpoints::room_terrain::TerrainType;
            //                 let next_tile_id = tile_walk.next(&mut ids.tiles, &mut id_gen);
            //                 let canvas = Canvas::new().color(match *terrain {
            //                     TerrainType::Plains => color::LIGHT_GREY,
            //                     TerrainType::Swamp => color::DARK_GREEN,
            //                     TerrainType::Wall | TerrainType::SwampyWall => color::DARK_GREY,
            //                 });
            //                 (next_tile_id, canvas)
            //             })
            //             .collect::<Vec<_>>();

            //         let canvas = Canvas::new().flow_right(&columns);
            //         (next_row_id, canvas)
            //     })
            //     .collect::<Vec<_>>();

            // body = Canvas::new()
            //     .color(color::BLACK)
            //     .border(0.0)
            //     .flow_down(&rows);
        }
    }

    Ok(())
}
