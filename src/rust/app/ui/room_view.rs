use std::default::Default;

use conrod::{color, Colorable, Positionable, Widget, Borderable, Rect};
use conrod::widget::*;

use screeps_api;

use network;

use super::super::AppCell;
use super::{GraphicsState, PanelStates, frame, left_panel_available, AdditionalRender};
use self::room_view_widget::ScrollableRoomView;

#[derive(Debug)]
pub struct RoomViewState<T: network::ScreepsConnection = network::ThreadedHandler> {
    network: T,
    scroll: (f64, f64),
    panels: PanelStates,
}

impl<T: network::ScreepsConnection> RoomViewState<T> {
    pub fn new(network: T) -> Self {
        RoomViewState {
            network: network,
            scroll: (0.0, 0.0),
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
        .color(color::TRANSPARENT)
        .border(0.0);

    frame(ui, ids, ids.body, body);

    left_panel_available(ui, ids, &mut state.panels, update);

    // scrolling
    if let Some(scroll) = ScrollableRoomView::new(state.scroll).set(ids.room_scroll_widget, ui) {
        state.scroll = scroll;
    }

    // display rect
    Rectangle::fill(ui.wh_of(ids.body).unwrap())
        .rgba(1.0, 0.0, 0.0, 0.5)
        .middle_of(ids.body)
        .graphics_for(ids.room_scroll_widget)
        .set(ids.room_display, ui);

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

        let view_rect = ui.rect_of(ids.room_display).expect("expected room_display to have a rect");

        let (map_scroll_x, map_scroll_y) = state.scroll;
        let size = 200.0f64;

        let first_room_x = (map_scroll_x / size).floor() as i32;
        let first_room_y = (map_scroll_y / size).floor() as i32;
        let initial_room = screeps_api::RoomName {
            x_coord: first_room_x,
            y_coord: first_room_y,
        };
        let extra_scroll_x = -(map_scroll_x % size);
        let extra_scroll_y = -(map_scroll_y % size);
        let count_x = (view_rect.w() / size).ceil() as i32;
        let count_y = (view_rect.h() / size).ceil() as i32;
        let (count_x, extra_scroll_x) = if extra_scroll_x > 0.0 {
            (count_x + 1, extra_scroll_x - size)
        } else {
            (count_x, extra_scroll_x)
        };
        let (count_y, extra_scroll_y) = if extra_scroll_y > 0.0 {
            (count_y + 1, extra_scroll_y - size)
        } else {
            (count_y, extra_scroll_y)
        };
        debug!("map: ({}, {}) initial room: {}. extra scroll: ({}, {}). count: ({}, {})",
               map_scroll_x,
               map_scroll_y,
               initial_room,
               extra_scroll_x,
               extra_scroll_y,
               count_x,
               count_y);
        let rooms = (0..count_x).flat_map(move |rel_x| {
                (0..count_y).map(move |rel_y| {
                    let room_name = initial_room + (rel_x, rel_y);

                    let x = view_rect.left() + extra_scroll_x + (rel_x as f64) * size;
                    let y = view_rect.bottom() + extra_scroll_y + (rel_y as f64) * size;

                    (room_name, Rect::from_corners([x, y], [x + size, y + size]))
                })
            })
            .flat_map(|(room_name, rect)| match net.room_terrain(room_name) {
                Ok(Some(terrain)) => {
                    debug!("found room terrain {}", room_name);
                    Some(Ok((rect, terrain.clone())))
                }
                Ok(None) => {
                    debug!("didn't find room terrain {}", room_name);
                    None
                }
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<Vec<(Rect, screeps_api::TerrainGrid)>, network::NotLoggedIn>>()?;

        if !rooms.is_empty() {
            *app.additional_rendering = Some(AdditionalRender::room_grid(ids.body, rooms));
        }
        // let room_name = screeps_api::RoomName::new("E0N0").unwrap();
        // if let Some(terrain) = net.room_terrain(room_name)? {
        //     debug!("found terrain");
        //     *app.additional_rendering = Some(AdditionalRender::room(ids.body, room_name, terrain.clone()));
        // }
    }

    Ok(())
}

mod room_view_widget {
    use conrod::{widget, Widget};

    pub struct ScrollableRoomView {
        common: widget::CommonBuilder,
        style: Style,
        scroll: (f64, f64),
    }


    widget_style! {
        style Style {}
    }

    pub struct State {}

    impl ScrollableRoomView {
        pub fn new((scroll_x, scroll_y): (f64, f64)) -> Self {
            ScrollableRoomView {
                common: widget::CommonBuilder::new(),
                style: Style::new(),
                scroll: (scroll_x, scroll_y),
            }
        }
    }
    impl Widget for ScrollableRoomView {
        type State = State;
        type Style = Style;
        type Event = Option<(f64, f64)>;

        fn common(&self) -> &widget::CommonBuilder {
            &self.common
        }

        fn common_mut(&mut self) -> &mut widget::CommonBuilder {
            &mut self.common
        }

        fn init_state(&self, _: widget::id::Generator) -> State {
            State {}
        }

        fn style(&self) -> Style {
            self.style.clone()
        }

        /// Updates this widget. Returns an event of [scroll_x; scroll_y]
        fn update(self, args: widget::UpdateArgs<Self>) -> Option<(f64, f64)> {
            let widget::UpdateArgs { id, ui, mut state, .. } = args;

            let input = ui.widget_input(id);

            let (mut scroll_x, mut scroll_y) = self.scroll;
            let mut changed = false;

            for drag in input.drags().left() {
                scroll_x -= drag.delta_xy[0];
                scroll_y -= drag.delta_xy[1];
                changed = true;
            }

            if changed {
                // let the UI know the state has changed.
                state.update(|_| {});
                Some((scroll_x, scroll_y))
            } else {
                None
            }
        }
    }
}
