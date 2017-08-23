use std::default::Default;

use conrod::{color, Borderable, Colorable, Positionable, Rect, Sizeable, Widget, Labelable};
use conrod::widget::*;

use screeps_api;

use network::{self, SelectedRooms};
use rendering::MapViewOffset;

use app::AppCell;
use super::{frame, AdditionalRender, GraphicsState};
use super::left_panel::{left_panel_available, PanelStates};
use self::room_view_widget::ScrollableRoomView;

const ZOOM_MODIFIER: f64 = 1.0 / 500.0;
const MIN_ZOOM: f64 = 0.1;
const MAX_ZOOM: f64 = 10.0;

#[derive(Debug)]
pub struct RoomViewState {
    network: network::ThreadedHandler,
    scroll: ScrollState,
    panels: PanelStates,
    shard: Option<Option<String>>,
}

impl RoomViewState {
    pub fn new(network: network::ThreadedHandler) -> Self {
        RoomViewState {
            network: network,
            scroll: ScrollState::default(),
            panels: PanelStates::default(),
            shard: None,
        }
    }

    pub fn into_network(self) -> network::ThreadedHandler {
        self.network
    }
}

pub struct RoomViewIds {
    username_gcl_header: Id,
    display: Id,
    scroll_widget: Id,
    shard_dropdown: Id,
}

impl RoomViewIds {
    pub fn new(gen: &mut id::Generator) -> Self {
        RoomViewIds {
            username_gcl_header: gen.next(),
            display: gen.next(),
            scroll_widget: gen.next(),
            shard_dropdown: gen.next(),
        }
    }
}

pub fn create_ui(
    app: &mut AppCell,
    state: &mut RoomViewState,
    update: &mut Option<GraphicsState>,
) -> Result<(), network::NotLoggedIn> {
    let AppCell {
        ref mut ui,
        ref mut net_cache,
        ref mut ids,
        ..
    } = *app;

    let body = Canvas::new().color(color::TRANSPARENT).border(0.0);

    frame(ui, ids, ids.root.body, body);

    let left_open = left_panel_available(ui, ids, &mut state.panels, update);

    // scrolling
    let scroll_update = ScrollableRoomView::new()
        .wh(ui.wh_of(ids.root.body).unwrap())
        .middle_of(ids.root.body)
        .set(ids.room_view.scroll_widget, ui);

    // display rect
    Rectangle::fill(ui.wh_of(ids.root.body).unwrap())
        .color(color::TRANSPARENT)
        .middle_of(ids.root.body)
        .graphics_for(ids.room_view.scroll_widget)
        .set(ids.room_view.display, ui);

    let mut bail = false;

    {
        let mut net = net_cache.align(&mut state.network, |err| match err {
            network::ErrorEvent::NotLoggedIn => bail = true,
            other => {
                // TODO: do a "notification bar" side thing in the app with these.
                warn!("network error occurred: {}", other);
            }
        });

        if left_open {
            let shard_list = net.shard_list();
            match shard_list {
                Some(Some(shards)) => {
                    DropDownList::new(shards, None)
                        .parent(ids.left_panel.open_panel_canvas)
                        .top_left_of(ids.left_panel.open_panel_canvas)
                        .scrollbar_on_top()
                        .left_justify_label()
                        .max_visible_height(150f64)
                        .small_font(ui)
                        .set(ids.room_view.shard_dropdown, ui);
                }
                Some(None) => {}
                None => {}
            }
        }


        if let Some(info) = net.my_info() {
            Text::new(&format!("{} - GCL {}", info.username, screeps_api::gcl_calc(info.gcl_points)))
                // style
                .font_size(ui.theme.font_size_small)
                .right_justify()
                .no_line_wrap()
                // position
                .mid_right_with_margin_on(ids.root.header, 10.0)
                .set(ids.room_view.username_gcl_header, ui);
        }

        let view_rect = ui.rect_of(ids.room_view.display)
            .expect("expected room_display to have a rect");

        if let Some(update) = scroll_update {
            state.scroll.update(view_rect, update);
        }

        let ScrollState {
            scroll_x: saved_room_scroll_x,
            scroll_y: saved_room_scroll_y,
            zoom_factor,
            selected_room,
            ..
        } = state.scroll;

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
        debug!(
            "scroll_state: ({:?}) initial room: {}. extra scroll: ({}, {}). count: ({}, {})",
            state.scroll,
            initial_room,
            extra_scroll_x,
            extra_scroll_y,
            count_x,
            count_y
        );

        // fetch rooms just outside the boundary as well so we can have smoother scrolling
        let rooms_to_fetch = SelectedRooms::new((initial_room - (1, 1))..(initial_room + (count_x + 1, count_y + 1)));

        let room_data = net.view_rooms(rooms_to_fetch, selected_room).clone();

        let rooms_to_view = SelectedRooms::new(initial_room..(initial_room + (count_x, count_y)));
        let offset = MapViewOffset::new(extra_scroll_x, extra_scroll_y, room_size);

        *app.additional_rendering = Some(AdditionalRender::map_view(ids.root.body, rooms_to_view, room_data, offset));
    }

    if bail {
        Err(network::NotLoggedIn)
    } else {
        Ok(())
    }
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
    /// The horizontal scroll, in fractional rooms. 1 is 1 room.
    scroll_x: f64,
    /// The vertical scroll, in fractional rooms. 1 is 1 room.
    scroll_y: f64,
    /// The zoom factor, 1.0 is a room the same size as the minimum screen dimension.
    zoom_factor: f64,
    /// The room name currently selected.
    selected_room: Option<screeps_api::RoomName>,
}

impl Default for ScrollState {
    fn default() -> Self {
        ScrollState {
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom_factor: 1.0,
            selected_room: None,
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
    /// If the screen was clicked, the relative (x, y) that it was clicked at.
    clicked: Option<(f64, f64)>,
}

impl ScrollState {
    fn room_name_and_xy_from_rel_pos(
        &self,
        view_rect: Rect,
        mouse_rel_x: f64,
        mouse_rel_y: f64,
    ) -> (screeps_api::RoomName, (f64, f64)) {
        let abs_mouse_x = view_rect.w() / 2.0 + mouse_rel_x;
        let abs_mouse_y = view_rect.h() / 2.0 + mouse_rel_y;

        let ScrollState {
            scroll_x: saved_room_scroll_x,
            scroll_y: saved_room_scroll_y,
            zoom_factor,
            ..
        } = *self;

        let room_size = view_rect.w().min(view_rect.h()) * zoom_multiplier_from_factor(zoom_factor);

        let room_scroll_x = saved_room_scroll_x - (view_rect.w() / room_size / 2.0) + (abs_mouse_x / room_size);
        let room_scroll_y = saved_room_scroll_y - (view_rect.h() / room_size / 2.0) + (abs_mouse_y / room_size);

        let initial_room = screeps_api::RoomName {
            x_coord: room_scroll_x.floor() as i32,
            y_coord: room_scroll_y.floor() as i32,
        };

        return (initial_room, (0.0, 0.0));

        // let extra_scroll_x = -(room_scroll_x % 1.0) * room_size;
        // let extra_scroll_y = -(room_scroll_y % 1.0) * room_size;
        // let extra_scroll_x = if extra_scroll_x > 0.0 {
        //     extra_scroll_x - room_size
        // } else {
        //     extra_scroll_x
        // };
        // let extra_scroll_y = if extra_scroll_y > 0.0 {
        //     extra_scroll_y - room_size
        // } else {
        //     extra_scroll_y
        // };
        // let count_x = ((view_rect.w() - extra_scroll_x) / room_size).ceil() as i32;
        // let count_y = ((view_rect.h() - extra_scroll_y) / room_size).ceil() as i32;

        // let extra_scroll_x = -(room_scroll_x % 1.0) * room_size;
        // let extra_scroll_y = -(room_scroll_y % 1.0) * room_size;
        // let extra_scroll_x = if extra_scroll_x > 0.0 {
        //     extra_scroll_x - room_size
        // } else {
        //     extra_scroll_x
        // };
        // let extra_scroll_y = if extra_scroll_y > 0.0 {
        //     extra_scroll_y - room_size
        // } else {
        //     extra_scroll_y
        // };

        // unimplemented!()
    }

    fn update(&mut self, view_rect: Rect, update: ScrollUpdate) {
        let room_size_unit = view_rect.w().min(view_rect.h());

        if let Some((clicked_x, clicked_y)) = update.clicked {
            let (room_clicked, _) = self.room_name_and_xy_from_rel_pos(view_rect, clicked_x, clicked_y);

            info!("Clicked {}", room_clicked);

            self.selected_room = Some(room_clicked);
        }

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

    #[derive(WidgetCommon)]
    pub(super) struct ScrollableRoomView {
        #[conrod(common_builder)] common: widget::CommonBuilder,
        style: Style,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq, WidgetStyle)]
    pub(super) struct Style {}

    pub(super) struct State {}

    impl ScrollableRoomView {
        pub(super) fn new() -> Self {
            ScrollableRoomView {
                common: widget::CommonBuilder::default(),
                style: Style::default(),
            }
        }
    }
    impl Widget for ScrollableRoomView {
        type State = State;
        type Style = Style;
        type Event = Option<ScrollUpdate>;

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

            let widget::UpdateArgs { id, ui, state, .. } = args;

            let input = ui.widget_input(id);

            let mut changed = false;
            let mut update = ScrollUpdate::default();

            for event in input.events() {
                match event {
                    Event::Drag(drag) => if drag.button == MouseButton::Left {
                        update.scrolled_map_x -= drag.delta_xy[0];
                        update.scrolled_map_y -= drag.delta_xy[1];
                        changed = true;
                    },
                    Event::Scroll(scroll) => if scroll.modifiers.is_empty() {
                        update.zoom_change -= scroll.y;
                        changed = true;
                    },
                    Event::Click(click) => if click.button == MouseButton::Left {
                        update.clicked = Some((click.xy[0], click.xy[1]));
                    },
                    _ => {}
                }
            }
            if update.zoom_change != 0.0 {
                let mouse = input
                    .mouse()
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
