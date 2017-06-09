use conrod::{self, color, Colorable, Labelable, Positionable, Sizeable, Widget, Borderable};
use conrod::widget::*;

use time;

use debugging::{FailureUnwrap, FailStage};
use network::{self, ScreepsConnection, Request};

use super::super::AppCell;
use super::{GraphicsState, frame, HEADER_HEIGHT};
use super::room_view::RoomViewState;

const LOGIN_WIDTH: conrod::Scalar = 300.0;
const LOGIN_HEIGHT: conrod::Scalar = 200.0;

const LOGIN_PADDING: conrod::Scalar = 10.0;

const LOGIN_LOWER_SECTION_HEIGHT: conrod::Scalar = (LOGIN_HEIGHT - HEADER_HEIGHT) / 3.0;

pub struct LoginScreenState<T: network::ScreepsConnection = network::ThreadedHandler> {
    network: Option<T>,
    pending_since: Option<time::Tm>,
    username: String,
    password: String,
}

impl<T: network::ScreepsConnection> Default for LoginScreenState<T> {
    fn default() -> Self {
        LoginScreenState {
            network: None,
            pending_since: None,
            // the UI username and password boxes are empty by default.
            username: String::new(),
            password: String::new(),
        }
    }
}

impl<T: network::ScreepsConnection> LoginScreenState<T> {
    pub fn new(network: T) -> Self {
        LoginScreenState { network: Some(network), ..LoginScreenState::default() }
    }

    pub fn into_network(self) -> Option<T> {
        self.network
    }
}

impl ::std::fmt::Debug for LoginScreenState {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.debug_struct("LoginScreenState")
            .field("network", &self.network)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

pub fn create_ui(app: &mut AppCell, state: &mut LoginScreenState, update: &mut Option<GraphicsState>) {
    if let Some(ref mut network) = state.network {
        app.net_cache.align(network, |event| {
            warn!("Network error: {}", event);
        });
    }

    if app.net_cache.login_state() == network::LoginState::LoggedIn {
        if let Some(network) = state.network.take() {
            debug!("Logged in, moving out.");
            let mut new_state = RoomViewState::new(network);
            let mut temp_secondary_update = None;
            super::room_view::create_ui(app, &mut new_state, &mut temp_secondary_update)
                .expect("Just logged in, yet login error occurs?");
            *update = Some(temp_secondary_update.unwrap_or_else(|| GraphicsState::RoomView(new_state)));
            return;
        }
    }

    let AppCell { ref mut ui, ref display, ref mut ids, .. } = *app;

    use conrod::widget::text_box::Event as TextBoxEvent;

    let body = Canvas::new()
        .color(color::CHARCOAL)
        .border(0.0);
    frame(ui, ids, ids.body, body);

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
                debug!("sending login request to existing network.");
                net.send(Request::login(&*state.username, &*state.password))
                    .expect("Cannot receive login error for login request.");
            }
            None => {
                debug!("sending login request to new network.");
                let proxy = display.get_window()
                    .uw(FailStage::Runtime, "could not find window, headless?")
                    .create_window_proxy();
                let mut network = network::ThreadedHandler::new(proxy);
                network.send(network::Request::login(&*state.username, &*state.password))
                    .expect("Cannot receive login error for login request.");
                state.network = Some(network);
                state.pending_since = Some(time::now_utc());
            }
        }
    }
}
