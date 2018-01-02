use std::collections::VecDeque;

use conrod::{color, Borderable, Colorable, Positionable, Rect, Sizeable, Widget};
use conrod::widget::*;

use screeps_api;

use screeps_rs_network::SelectedRooms;
use ui_state::{self, Event as UiEvent, MapClickEvent, MapPanEvent, MapScreenState, MapZoomEvent, ScrollState};
use rendering::MapViewOffset;

use app::AppCell;
use super::{frame, AdditionalRender};
use super::left_panel::left_panel_available;
use self::room_view_widget::ScrollableRoomView;
use map_view_utils::zoom_multiplier_from_factor;

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

pub fn create_ui(app: &mut AppCell, state: &MapScreenState, mut update: &mut VecDeque<UiEvent>) {
    let AppCell {
        ref mut ui,
        ref mut net_cache,
        ref mut ids,
        ..
    } = *app;

    let body = Canvas::new().color(color::TRANSPARENT).border(0.0);

    frame(ui, ids, ids.root.body, body);

    left_panel_available(ui, ids, &state.panels, update);

    // scrolling
    let scroll_result = ScrollableRoomView::new()
        .wh(ui.wh_of(ids.root.body).unwrap())
        .middle_of(ids.root.body)
        .set(ids.room_view.scroll_widget, ui);

    // display rect
    Rectangle::fill(ui.wh_of(ids.root.body).unwrap())
        .color(color::TRANSPARENT)
        .middle_of(ids.root.body)
        .graphics_for(ids.room_view.scroll_widget)
        .set(ids.room_view.display, ui);

    if state.panels.left == ui_state::MenuState::Open {
        let shard_list = net_cache.shard_list();
        match shard_list {
            Some(Some(shards)) => {
                let mut text = String::new();
                for shard_info in shards {
                    use std::fmt::Write;
                    write!(text, "{}\n", shard_info.as_ref()).expect("writing plain string to plain string");
                }
                Text::new(&text)
                    .font_size(ui.theme.font_size_medium)
                    .right_justify()
                    .no_line_wrap()
                    .top_left_of(ids.left_panel.open_panel_canvas)
                    .set(ids.room_view.shard_dropdown, ui);
            }
            Some(None) => {
                Text::new("<no shards>")
                    .font_size(ui.theme.font_size_medium)
                    .right_justify()
                    .no_line_wrap()
                    .top_left_of(ids.left_panel.open_panel_canvas)
                    .set(ids.room_view.shard_dropdown, ui);
            }
            None => {}
        }
    }

    if let Some(info) = net_cache.my_info() {
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

    scroll_result.map(|scroll_update| scroll_update.into_events(view_rect, &mut update));

    let ScrollState {
        scroll_x: saved_room_scroll_x,
        scroll_y: saved_room_scroll_y,
        zoom_factor,
        selected_room,
        ..
    } = state.map_scroll;

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
        state.map_scroll, initial_room, extra_scroll_x, extra_scroll_y, count_x, count_y
    );

    // fetch rooms just outside the boundary as well so we can have smoother scrolling
    let rooms_to_fetch = SelectedRooms::new((initial_room - (1, 1))..(initial_room + (count_x + 1, count_y + 1)));

    let room_data = net_cache.view_rooms(rooms_to_fetch, selected_room).clone();

    let rooms_to_view = SelectedRooms::new(initial_room..(initial_room + (count_x, count_y)));
    let offset = MapViewOffset::new(extra_scroll_x, extra_scroll_y, room_size);

    *app.additional_rendering = Some(AdditionalRender::map_view(
        ids.root.body,
        rooms_to_view,
        room_data,
        offset,
    ));
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

impl ScrollUpdate {
    fn into_events(&self, view_rect: Rect, update: &mut VecDeque<UiEvent>) {
        if let Some(pos_tuple) = self.clicked {
            update.push_front(UiEvent::MapClick {
                view_rect: view_rect,
                event: MapClickEvent { clicked: pos_tuple },
            });
        }
        if self.zoom_change != 0.0 {
            update.push_front(UiEvent::MapZoom {
                view_rect: view_rect,
                event: MapZoomEvent {
                    zoom_change: self.zoom_change,
                    zoom_mouse_rel_x: self.zoom_mouse_rel_x,
                    zoom_mouse_rel_y: self.zoom_mouse_rel_y,
                },
            });
        }
        if self.scrolled_map_x != 0.0 || self.scrolled_map_y != 0.0 {
            update.push_front(UiEvent::MapPan {
                view_rect: view_rect,
                event: MapPanEvent {
                    scrolled_map_x: self.scrolled_map_x,
                    scrolled_map_y: self.scrolled_map_y,
                },
            });
        }
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

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
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
                        debug!("drag update");
                        update.scrolled_map_x -= drag.delta_xy[0];
                        update.scrolled_map_y -= drag.delta_xy[1];
                        changed = true;
                    },
                    Event::Scroll(scroll) => if scroll.modifiers.is_empty() {
                        debug!("scroll update");
                        update.zoom_change -= scroll.y;
                        changed = true;
                    },
                    Event::Click(click) => if click.button == MouseButton::Left {
                        debug!("click update");
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
