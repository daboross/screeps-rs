use std::default::Default;

use conrod::{self, color, Colorable, Labelable, Positionable, Sizeable, Widget, Borderable};
use conrod::widget::*;

use time;
use screeps_api;

use debugging::{FailureUnwrap, FailStage};
use network::{self, NetworkRequests, Request};
use glue::AppCell;

const HEADER_HEIGHT: conrod::Scalar = 30.0;

const LOGIN_WIDTH: conrod::Scalar = 300.0;
const LOGIN_HEIGHT: conrod::Scalar = 200.0;

const LOGIN_PADDING: conrod::Scalar = 10.0;

const LOGIN_LOWER_SECTION_HEIGHT: conrod::Scalar = (LOGIN_HEIGHT - HEADER_HEIGHT) / 3.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MenuState {
    Open,
    Closed,
}
impl Default for MenuState {
    fn default() -> Self { MenuState::Closed }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PanelStates {
    left: MenuState,
}

// Note: username and password defaults are empty strings, and this makes sense.
#[derive(Default)]
pub struct LoginScreenState {
    network: Option<network::NetworkRequests>,
    pending_since: Option<time::Tm>,
    username: String,
    password: String,
}

impl ::std::fmt::Debug for LoginScreenState {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.debug_struct("LoginScreenState")
            .field("network", &self.network)
            .field("username", &self.username)
            .field("password", &"redacted")
            .finish()
    }
}

#[derive(Debug)]
pub struct MainScreenState {
    network: network::NetworkRequests,
    panels: PanelStates,
}

#[derive(Debug)]
pub enum GraphicsState {
    LoginScreen(LoginScreenState),
    MainScreen(MainScreenState),
    Exit,
}

impl GraphicsState {
    pub fn login_screen() -> Self { GraphicsState::LoginScreen(LoginScreenState::default()) }

    pub fn main_screen(network: network::NetworkRequests) -> Self {
        GraphicsState::MainScreen(MainScreenState {
            panels: PanelStates::default(),
            network: network,
        })
    }
}

pub fn create(app: &mut AppCell, state: &mut GraphicsState) {
    let mut update = None;

    match *state {
        GraphicsState::LoginScreen(ref mut inner) => login_screen(app, inner, &mut update),
        GraphicsState::MainScreen(ref mut inner) => main_screen(app, inner, &mut update),
        GraphicsState::Exit => panic!("Should have exited."),
    }

    if let Some(inner) = update {
        *state = inner;
    }
}

fn login_screen(app: &mut AppCell, state: &mut LoginScreenState, update: &mut Option<GraphicsState>) {
    if let Some(ref mut network) = state.network {
        app.net_cache.align(network);
    }

    if app.net_cache.login_state() == network::LoginState::LoggedIn {
        if let Some(network) = state.network.take() {
            let mut new_state = MainScreenState {
                network: network,
                panels: PanelStates::default(),
            };
            let mut temp_secondary_update = None;
            main_screen(app, &mut new_state, &mut temp_secondary_update);
            *update = Some(temp_secondary_update.unwrap_or_else(|| GraphicsState::MainScreen(new_state)));
            return;
        }
    }

    let AppCell { ref mut ui, ref display, ref mut image_map, ref mut ids, ref mut renderer, ref mut net_cache, .. } =
        *app;

    use conrod::widget::text_box::Event as TextBoxEvent;

    let body = Canvas::new()
        .color(color::CHARCOAL)
        .border(0.0);
    frame(ui, ids, body);

    let header_canvas = Canvas::new()
        // style
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .length(HEADER_HEIGHT);

    let bottom_template = Canvas::new()
        // style
        .color(color::DARK_GREY)
        .border_color(color::BLACK);

    // root canvas
    Canvas::new()
        // style
        .color(color::GREY)
        .border(2.0)
        .border_color(color::DARK_GREY)
        .w_h(LOGIN_WIDTH, LOGIN_HEIGHT)
        // behavior
        .flow_down(&[
            (ids.login_header_canvas, header_canvas),
            (ids.login_username_canvas, bottom_template.clone()),
            (ids.login_password_canvas, bottom_template.clone()),
            (ids.login_submit_canvas, bottom_template),
        ])
        // place
        .floating(true)
        .mid_top_of(ids.root)
        .down_from(ids.header, ui.window_dim()[1] / 4.0 - HEADER_HEIGHT)
        // set
        .set(ids.login_canvas, ui);

    fn textbox_field(text: &mut String,
                     parent: Id,
                     id: Id,
                     width: conrod::Scalar,
                     hide: bool,
                     ui: &mut conrod::UiCell)
                     -> bool {
        let events = TextBox::new(&text)
            // style
            .w_h(width, LOGIN_LOWER_SECTION_HEIGHT - LOGIN_PADDING * 2.0)
            .font_size(ui.theme.font_size_small)
            .left_justify()
            .pad_text(5.0)
            .hide_with_char(if hide { Some('*') } else { None })
            // position
            .mid_right_with_margin_on(parent, 10.0)
            .set(id, ui);

        let mut updated_string = None;
        let mut enter_pressed = false;

        for event in events.into_iter() {
            match event {
                TextBoxEvent::Update(s) => {
                    updated_string = Some(s);
                }
                TextBoxEvent::Enter => {
                    enter_pressed = true;
                    break;
                }
            }
        }
        if let Some(s) = updated_string {
            *text = s;
        }
        enter_pressed
    }

    // username label
    Text::new("username")
        // style
        .font_size(ui.theme.font_size_small)
        .center_justify()
        .no_line_wrap()
        // position
        .mid_left_with_margin_on(ids.login_username_canvas, LOGIN_PADDING)
        .set(ids.login_username_label, ui);

    // password label
    Text::new("password")
        // style
        .font_size(ui.theme.font_size_small)
        .center_justify()
        .no_line_wrap()
        // position
        .mid_left_with_margin_on(ids.login_password_canvas, LOGIN_PADDING)
        .set(ids.login_password_label, ui);

    let label_width = match (ui.w_of(ids.login_username_label), ui.w_of(ids.login_password_label)) {
        (Some(w1), Some(w2)) => conrod::Scalar::max(w1, w2),
        (Some(w), None) | (None, Some(w)) => w,
        (None, None) => LOGIN_WIDTH / 2.0 - LOGIN_PADDING * 1.5,
    };

    // Username field
    let username_enter_pressed = textbox_field(&mut state.username,
                                               ids.login_username_canvas,
                                               ids.login_username_textbox,
                                               LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
                                               false,
                                               ui);

    // Password field
    let password_enter_pressed = textbox_field(&mut state.password,
                                               ids.login_password_canvas,
                                               ids.login_password_textbox,
                                               LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
                                               true,
                                               ui);

    let submit_pressed = Button::new()
        // style
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .w_h(LOGIN_WIDTH / 2.0 - 30.0, LOGIN_LOWER_SECTION_HEIGHT - LOGIN_PADDING * 2.0)
        // label
        .label("submit")
        .small_font(ui)
        .center_justify_label()
        // position
        .mid_right_with_margin_on(ids.login_submit_canvas, 10.0)
        .set(ids.login_submit_button, ui)
        // now TimesClicked
        .was_clicked();

    let exit_pressed = Button::new()
        // style
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .w_h(LOGIN_WIDTH / 2.0 - 30.0, LOGIN_LOWER_SECTION_HEIGHT - LOGIN_PADDING * 2.0)
        // label
        .label("exit")
        .small_font(ui)
        .center_justify_label()
        // position
        .mid_left_with_margin_on(ids.login_submit_canvas, 10.0)
        .set(ids.login_exit_button, ui)
        // now TimesClicked
        .was_clicked();

    if exit_pressed {
        *update = Some(GraphicsState::Exit);
    } else if (submit_pressed || password_enter_pressed || username_enter_pressed) && state.username.len() > 0 &&
              state.password.len() > 0 {
        match state.network {
            Some(ref mut net) => {
                net.send(Request::login(&*state.username, &*state.password));
            }
            None => {
                let proxy = display.get_window()
                    .uw(FailStage::Runtime, "could not find window, headless?")
                    .create_window_proxy();
                let network = NetworkRequests::new(proxy, state.username.clone(), state.password.clone());
                state.network = Some(network);
                state.pending_since = Some(time::now_utc());
            }
        }
    }
}

fn main_screen<'a>(app: &'a mut AppCell,
                       state: &'a mut MainScreenState,
                       update: &'a mut Option<GraphicsState>) {
    let AppCell { ref mut ui, ref mut net_cache, ref ids, ref display, .. } = *app;
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

fn left_panel_available(ui: &mut conrod::UiCell,
                        ids: &Ids,
                        state: &mut PanelStates,
                        update: &mut Option<GraphicsState>) {
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
    match state.left {
        MenuState::Open => {
            left_panel_panel_open(ui, ids, update);

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

                state.left = MenuState::Closed;
            }
        }
        MenuState::Closed => {
            if left_toggle_clicks % 2 == 1 {
                state.left = MenuState::Open;
            }
        }
    }
}

fn left_panel_panel_open(ui: &mut conrod::UiCell, ids: &Ids, _update: &mut Option<GraphicsState>) {
    Canvas::new()
        // style
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .w_h(300.0, ui.window_dim()[1] - HEADER_HEIGHT)
        // behavior
        .scroll_kids_vertically()
        // place
        .floating(true)
        .mid_left_of(ids.root)
        .down_from(ids.left_panel_toggle, 0.0)
        .set(ids.left_panel_canvas, ui);
}

fn frame(ui: &mut conrod::UiCell, ids: &Ids, body: Canvas) {
    let header = Canvas::new()
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .length(HEADER_HEIGHT);

    Canvas::new()
        .border(0.0)
        .flow_down(&[(ids.header, header), (ids.body, body)])
        .set(ids.root, ui);
}

widget_ids! {
    pub struct Ids {
        // global
        root,
        header,
        body,

        // Main screen
        left_panel_toggle,
        left_panel_canvas,

        username_gcl_header,

        // Login screen
        login_canvas,
        login_header_canvas,

        login_username_canvas,
        login_username_textbox,
        login_username_label,

        login_password_canvas,
        login_password_textbox,
        login_password_label,

        login_submit_canvas,
        login_exit_button,
        login_submit_button,
    }
}
