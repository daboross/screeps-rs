use std::default::Default;

use conrod::{color, Colorable, Positionable, Widget, Rect, Borderable, Sizeable};
use conrod::widget::*;

use screeps_api;

use network;

use super::super::AppCell;
use super::{GraphicsState, PanelStates, frame, left_panel_available, AdditionalRender};
use self::room_view_widget::ScrollableRoomView;

const ZOOM_MODIFIER: f64 = 1.0 / 500.0;
const MIN_ZOOM: f64 = 0.04;
const MAX_ZOOM: f64 = 10.0;

#[derive(Debug)]
pub struct RoomViewState<T: network::ScreepsConnection = network::ThreadedHandler> {
    network: T,
    scroll: ScrollState,
    panels: PanelStates,
}

impl<T: network::ScreepsConnection> RoomViewState<T> {
    pub fn new(network: T) -> Self {
        RoomViewState {
            network: network,
            scroll: ScrollState::default(),
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
    let scroll_update = ScrollableRoomView::new()
        .wh(ui.wh_of(ids.body).unwrap())
        .middle_of(ids.body)
        .set(ids.room_scroll_widget, ui);

    // display rect
    Rectangle::fill(ui.wh_of(ids.body).unwrap())
        .color(color::TRANSPARENT)
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

        if let Some(update) = scroll_update {
            state.scroll.update(view_rect, update);
        }

        let ScrollState { scroll_x: saved_room_scroll_x, scroll_y: saved_room_scroll_y, zoom_factor, .. } =
            state.scroll;

        let room_size = view_rect.w().min(view_rect.h()) * zoom_multiplier_from_factor(zoom_factor);

        let room_scroll_x = saved_room_scroll_x - (view_rect.w() / room_size / 2.0);
        let room_scroll_y = saved_room_scroll_y - (view_rect.h() / room_size / 2.0);

        let initial_room = screeps_api::RoomName {
            x_coord: room_scroll_x.floor() as i32,
            y_coord: room_scroll_y.floor() as i32,
        };

        let extra_scroll_x = -(room_scroll_x % 1.0) * room_size;
        let extra_scroll_y = -(room_scroll_y % 1.0) * room_size;
        let extra_scroll_x = if extra_scroll_x > 0.0 {
            extra_scroll_x - room_size
        } else {
            extra_scroll_x
        };
        let extra_scroll_y = if extra_scroll_y > 0.0 {
            extra_scroll_y - room_size
        } else {
            extra_scroll_y
        };
        let count_x = ((view_rect.w() - extra_scroll_x) / room_size).ceil() as i32;
        let count_y = ((view_rect.h() - extra_scroll_y) / room_size).ceil() as i32;
        debug!("scroll_state: ({:?}) initial room: {}. extra scroll: ({}, {}). count: ({}, {})",
               state.scroll,
               initial_room,
               extra_scroll_x,
               extra_scroll_y,
               count_x,
               count_y);

        let rooms = (0..count_x).flat_map(move |rel_x| {
                (0..count_y).map(move |rel_y| {
                    let room_name = initial_room + (rel_x, rel_y);

                    let x = view_rect.left() + extra_scroll_x + (rel_x as f64) * room_size;
                    let y = view_rect.bottom() + extra_scroll_y + (rel_y as f64) * room_size;

                    (room_name, Rect::from_corners([x, y], [x + room_size, y + room_size]))
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

        // fetch rooms just outside the boundary as well so we can have smoother moving
        for &rel_x in [-1, count_x + 1].iter() {
            for rel_y in -1..count_y + 1 {
                let _ = net.room_terrain(initial_room + (rel_x, rel_y));
            }
        }
        for &rel_y in [-1, count_y + 1].iter() {
            for rel_x in -1..count_x + 1 {
                let _ = net.room_terrain(initial_room + (rel_x, rel_y));
            }
        }

        if !rooms.is_empty() {
            *app.additional_rendering = Some(AdditionalRender::room_grid(ids.body, rooms));
        }
    }

    Ok(())
}

#[inline(always)]
fn zoom_multiplier_from_factor(zoom_factor: f64) -> f64 {
    zoom_factor.powf(2.0)
}

#[inline(always)]
fn bound_zoom(zoom_factor: f64) -> f64 {
    zoom_factor.powf(2.0).min(MAX_ZOOM).max(MIN_ZOOM).powf(0.5)
}

#[derive(Copy, Clone, Debug)]
pub struct ScrollState {
    // The horizontal scroll, in fractional rooms. 1 is 1 room.
    scroll_x: f64,
    // The vertical scroll, in fractional rooms. 1 is 1 room.
    scroll_y: f64,
    zoom_factor: f64,
}

impl Default for ScrollState {
    fn default() -> Self {
        ScrollState {
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom_factor: 2.0,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct ScrollUpdate {
    /// The number of pixels scrolled horizontally.
    scrolled_map_x: f64,
    /// The number of pixels scrolled vertically.
    scrolled_map_y: f64,
    /// The scroll change amount (unknown unit).
    zoom_change: f64,
    /// If zoom_change != 0.0, this is the x position the mouse was at, relative to the center of the widget.
    zoom_mouse_rel_x: f64,
    /// If zoom_change != 0.0, this is the y position the mouse was at, relative to the center of the widget.
    zoom_mouse_rel_y: f64,
}

impl ScrollState {
    fn update(&mut self, view_rect: Rect, update: ScrollUpdate) {
        let room_size_unit = view_rect.w().min(view_rect.h());
        if update.zoom_change != 0.0 {
            let abs_mouse_x = view_rect.w() / 2.0 + update.zoom_mouse_rel_x;
            let abs_mouse_y = view_rect.h() / 2.0 + update.zoom_mouse_rel_y;

            let new_zoom_factor = bound_zoom(self.zoom_factor + update.zoom_change * ZOOM_MODIFIER);

            if self.zoom_factor != new_zoom_factor {
                let room_pixel_size = room_size_unit * zoom_multiplier_from_factor(self.zoom_factor);
                let new_room_pixel_size = room_size_unit * zoom_multiplier_from_factor(new_zoom_factor);

                let current_room_x = abs_mouse_x / room_pixel_size - (view_rect.w() / room_pixel_size / 2.0);
                let current_room_y = abs_mouse_y / room_pixel_size - (view_rect.h() / room_pixel_size / 2.0);

                let next_room_x = abs_mouse_x / new_room_pixel_size - (view_rect.w() / new_room_pixel_size / 2.0);
                let next_room_y = abs_mouse_y / new_room_pixel_size - (view_rect.h() / new_room_pixel_size / 2.0);

                self.scroll_x += current_room_x - next_room_x;
                self.scroll_y += current_room_y - next_room_y;
                self.zoom_factor = new_zoom_factor;
            }
        }

        let room_size = room_size_unit * zoom_multiplier_from_factor(self.zoom_factor);
        self.scroll_x += update.scrolled_map_x / room_size;
        self.scroll_y += update.scrolled_map_y / room_size;
    }
}

mod room_view_widget {
    use super::ScrollUpdate;
    use conrod::{widget, Widget};

    pub(super) struct ScrollableRoomView {
        common: widget::CommonBuilder,
        style: Style,
    }

    widget_style! {
        style Style {}
    }

    pub(super) struct State {}

    impl ScrollableRoomView {
        pub(super) fn new() -> Self {
            ScrollableRoomView {
                common: widget::CommonBuilder::new(),
                style: Style::new(),
            }
        }
    }
    impl Widget for ScrollableRoomView {
        type State = State;
        type Style = Style;
        type Event = Option<ScrollUpdate>;

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
        fn update(self, args: widget::UpdateArgs<Self>) -> Option<ScrollUpdate> {
            use conrod::event::Widget as Event;
            use conrod::input::MouseButton;

            let widget::UpdateArgs { id, ui, mut state, .. } = args;

            let input = ui.widget_input(id);

            let mut changed = false;
            let mut update = ScrollUpdate::default();

            for event in input.events() {
                match event {
                    Event::Drag(drag) => {
                        if drag.button == MouseButton::Left {
                            update.scrolled_map_x -= drag.delta_xy[0];
                            update.scrolled_map_y -= drag.delta_xy[1];
                            changed = true;
                        }
                    }
                    Event::Scroll(scroll) => {
                        if scroll.modifiers.is_empty() {
                            update.zoom_change -= scroll.y;
                            changed = true;
                        }
                    }
                    _ => {}
                }
            }
            if update.zoom_change != 0.0 {
                let mouse = input.mouse()
                    .expect("expected mouse to be captured by widget while scroll event is received");
                let rel_xy = mouse.rel_xy();
                update.zoom_mouse_rel_x = rel_xy[0];
                update.zoom_mouse_rel_y = rel_xy[1];
            }

            if changed {
                // let the UI know the state has changed.
                state.update(|_| {});
                Some(update)
            } else {
                None
            }
        }
    }
}
