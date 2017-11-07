use std::collections::VecDeque;

use conrod::{self, color, Borderable, Colorable, Labelable, Positionable, Sizeable, Widget};
use conrod::widget::*;

use super::{Ids, HEADER_HEIGHT};
use ui_state::{self, Event as UiEvent, MenuState};

pub struct LeftPanelIds {
    pub panel_toggle: Id,
    pub open_panel_canvas: Id,
}

impl LeftPanelIds {
    pub fn new(gen: &mut id::Generator) -> Self {
        LeftPanelIds {
            panel_toggle: gen.next(),
            open_panel_canvas: gen.next(),
        }
    }
}

pub fn left_panel_available(
    ui: &mut conrod::UiCell,
    ids: &Ids,
    state: &ui_state::PanelStates,
    update: &mut VecDeque<UiEvent>,
) {
    let left_toggle_clicks = Button::new()
        // style
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .w_h(100.0, HEADER_HEIGHT)
        // label
        .label("Screeps")
        .small_font(&ui)
        .left_justify_label()
        .label_color(color::WHITE)
        // place
        .parent(ids.root.header)
        .top_left_of(ids.root.header)
        .set(ids.left_panel.panel_toggle, ui)
        // now TimesClicked(u16)
        .0;

    match state.left {
        MenuState::Open => {
            left_panel_panel_open(ui, ids, update);

            if left_toggle_clicks % 2 == 1
                || left_toggle_clicks == 0
                    && ui.global_input()
                        .current
                        .mouse
                        .buttons
                        .pressed()
                        .next()
                        .is_some()
                    && ui.global_input()
                        .current
                        .widget_capturing_mouse
                        .or_else(|| ui.global_input().current.widget_under_mouse)
                        .map(|capturing| {
                            capturing != ids.left_panel.panel_toggle
                                && !ui.widget_graph().does_recursive_edge_exist(
                                    ids.left_panel.open_panel_canvas,
                                    capturing,
                                    |_| true,
                                )
                                && !ui.widget_graph().does_recursive_edge_exist(
                                    ids.left_panel.panel_toggle,
                                    capturing,
                                    |_| true,
                                )
                        })
                        .unwrap_or(true)
            {
                update.push_back(UiEvent::LeftMenuClosed);
            }
        }
        MenuState::Closed => if left_toggle_clicks % 2 == 1 {
            update.push_back(UiEvent::LeftMenuOpened);
        },
    }
}

pub fn left_panel_panel_open(ui: &mut conrod::UiCell, ids: &Ids, _update: &mut VecDeque<UiEvent>) {
    Canvas::new()
        // style
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .w_h(300.0, ui.window_dim()[1] - HEADER_HEIGHT)
        // behavior
        .scroll_kids_vertically()
        // place
        .floating(true)
        .mid_left_of(ids.root.root)
        .down_from(ids.left_panel.panel_toggle, 0.0)
        .set(ids.left_panel.open_panel_canvas, ui);
}
