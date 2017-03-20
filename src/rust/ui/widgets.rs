use conrod::{self, color, Colorable, Labelable, Positionable, Sizeable, Widget, Borderable};
use conrod::widget::*;

const HEADER_HEIGHT: conrod::Scalar = 30.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MenuState {
    Open,
    Closed,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MainScreenState {
    left_panel: MenuState,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GraphicsState {
    MainScreen(MainScreenState),
}

impl GraphicsState {
    pub fn new() -> Self { GraphicsState::MainScreen(MainScreenState { left_panel: MenuState::Closed }) }
}

pub fn create(ui: &mut conrod::UiCell, ids: &Ids, state: &mut GraphicsState) {
    let mut update = None;

    match *state {
        GraphicsState::MainScreen(ref mut inner) => main_screen(ui, ids, inner, &mut update),
    }

    if let Some(inner) = update {
        *state = inner;
    }
}

pub fn main_screen(ui: &mut conrod::UiCell,
                   ids: &Ids,
                   state: &mut MainScreenState,
                   update: &mut Option<GraphicsState>) {
    let header = Canvas::new()
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .length(HEADER_HEIGHT);

    let body = Canvas::new()
        .color(color::DARK_CHARCOAL)
        .border(5.0)
        .border_color(color::DARK_GREY);

    Canvas::new()
        .border(0.0)
        .flow_down(&[(ids.header, header), (ids.body, body)])
        .set(ids.master, ui);

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
        .parent(ids.header)
        .top_left_of(ids.header)
        .set(ids.left_panel_toggle, ui)
        // now TimesClicked(u16)
        .0;

    // left panel
    match state.left_panel {
        MenuState::Open => {
            create_left_panel(ui, ids, state, update);

            if left_toggle_clicks % 2 == 1 ||
               left_toggle_clicks == 0 && ui.global_input().current.mouse.buttons.pressed().next().is_some() &&
               ui.global_input()
                .current
                .widget_capturing_mouse
                .or_else(|| ui.global_input().current.widget_under_mouse)
                .map(|capturing| {
                    capturing != ids.left_panel_toggle &&
                    !ui.widget_graph().does_recursive_edge_exist(ids.left_panel_canvas, capturing, |_| true) &&
                    !ui.widget_graph().does_recursive_edge_exist(ids.left_panel_toggle, capturing, |_| true)
                })
                .unwrap_or(true) {

                state.left_panel = MenuState::Closed;
            }
        }
        MenuState::Closed => {
            if left_toggle_clicks % 2 == 1 {
                state.left_panel = MenuState::Open;
            }
        }
    }
}

pub fn create_left_panel(ui: &mut conrod::UiCell,
                         ids: &Ids,
                         _state: &MainScreenState,
                         _update: &mut Option<GraphicsState>) {
    Canvas::new()
        // style
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .w_h(300.0, ui.window_dim()[1] - HEADER_HEIGHT)
        // behavior
        .scroll_kids_vertically()
        // place
        .floating(true)
        .mid_left_of(ids.master)
        .down_from(ids.left_panel_toggle, 0.0)
        .set(ids.left_panel_canvas, ui);
}
widget_ids! {
    pub struct Ids {
        master,
        header,
        body,
        left_panel_toggle,
        left_panel_canvas,
    }
}
