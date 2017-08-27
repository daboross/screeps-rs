use conrod::{self, color, Borderable, Colorable, Labelable, Positionable, Sizeable, Widget};
use conrod::widget::*;

use time;

use screeps_rs_network::{self, ConnectionSettings, Request, ScreepsConnection};
use widgets::text_box::TextBox;
use NetworkHandler;

use app::AppCell;
use layout::{frame, GraphicsState, HEADER_HEIGHT};
use layout::room_view::RoomViewState;

const LOGIN_WIDTH: conrod::Scalar = 300.0;
const LOGIN_HEIGHT: conrod::Scalar = 200.0;

const LOGIN_PADDING: conrod::Scalar = 10.0;

const LOGIN_LOWER_SECTION_HEIGHT: conrod::Scalar = (LOGIN_HEIGHT - HEADER_HEIGHT) / 3.0;

#[derive(Default)] // the UI username and password boxes are empty by default.
pub struct LoginScreenState {
    network: Option<NetworkHandler>,
    pending_since: Option<time::Tm>,
    username: String,
    password: String,
}

impl LoginScreenState {
    pub fn new(network: NetworkHandler) -> Self {
        LoginScreenState {
            network: Some(network),
            ..LoginScreenState::default()
        }
    }

    pub fn into_network(self) -> Option<NetworkHandler> {
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

pub struct LoginIds {
    root: Id,
    header_canvas: Id,
    username_canvas: Id,
    username_textbox: Id,
    username_label: Id,
    password_canvas: Id,
    password_textbox: Id,
    password_label: Id,
    submit_canvas: Id,
    exit_button: Id,
    submit_button: Id,
}

impl LoginIds {
    pub fn new(gen: &mut id::Generator) -> Self {
        LoginIds {
            root: gen.next(),
            header_canvas: gen.next(),
            username_canvas: gen.next(),
            username_textbox: gen.next(),
            username_label: gen.next(),
            password_canvas: gen.next(),
            password_textbox: gen.next(),
            password_label: gen.next(),
            submit_canvas: gen.next(),
            exit_button: gen.next(),
            submit_button: gen.next(),
        }
    }
}

pub fn create_ui(app: &mut AppCell, state: &mut LoginScreenState, update: &mut Option<GraphicsState>) {
    if let Some(ref mut network) = state.network {
        app.net_cache.align(network, |event| {
            warn!("Network error: {}", event);
        });
    }

    if app.net_cache.login_state() == screeps_rs_network::LoginState::LoggedIn {
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

    let AppCell {
        ref mut ui,
        ref ids,
        ref notify,
        ..
    } = *app;

    use widgets::text_box::Event as TextBoxEvent;

    let body = Canvas::new().color(color::CHARCOAL).border(0.0);
    frame(ui, ids, ids.root.body, body);

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
            (ids.login.header_canvas, header_canvas),
            (ids.login.username_canvas, bottom_template.clone()),
            (ids.login.password_canvas, bottom_template.clone()),
            (ids.login.submit_canvas, bottom_template),
        ])
        // place
        .floating(true)
        .mid_top_of(ids.root.root)
        .down_from(ids.root.header, ui.window_dim()[1] / 4.0 - HEADER_HEIGHT)
        // set
        .set(ids.login.root, ui);

    fn textbox_field(
        text: &mut String,
        parent: Id,
        id: Id,
        width: conrod::Scalar,
        hide: bool,
        ui: &mut conrod::UiCell,
    ) -> bool {
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
        .mid_left_with_margin_on(ids.login.username_canvas, LOGIN_PADDING)
        .set(ids.login.username_label, ui);

    // password label
    Text::new("password")
        // style
        .font_size(ui.theme.font_size_small)
        .center_justify()
        .no_line_wrap()
        // position
        .mid_left_with_margin_on(ids.login.password_canvas, LOGIN_PADDING)
        .set(ids.login.password_label, ui);

    let label_width = match (ui.w_of(ids.login.username_label), ui.w_of(ids.login.password_label)) {
        (Some(w1), Some(w2)) => conrod::Scalar::max(w1, w2),
        (Some(w), None) | (None, Some(w)) => w,
        (None, None) => LOGIN_WIDTH / 2.0 - LOGIN_PADDING * 1.5,
    };

    // Username field
    let username_enter_pressed = textbox_field(
        &mut state.username,
        ids.login.username_canvas,
        ids.login.username_textbox,
        LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
        false,
        ui,
    );

    // Password field
    let password_enter_pressed = textbox_field(
        &mut state.password,
        ids.login.password_canvas,
        ids.login.password_textbox,
        LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
        true,
        ui,
    );

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
        .mid_right_with_margin_on(ids.login.submit_canvas, 10.0)
        .set(ids.login.submit_button, ui)
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
        .mid_left_with_margin_on(ids.login.submit_canvas, 10.0)
        .set(ids.login.exit_button, ui)
        // now TimesClicked
        .was_clicked();

    if exit_pressed {
        *update = Some(GraphicsState::Exit);
    } else if (submit_pressed || password_enter_pressed || username_enter_pressed) && state.username.len() > 0 &&
        state.password.len() > 0
    {
        // TODO: UI option for shard.
        let settings = ConnectionSettings::new(state.username.clone(), state.password.clone(), "shard0".to_owned());
        match state.network {
            Some(ref mut net) => {
                debug!("sending login request to existing network.");
                net.send(Request::change_settings(settings));
                net.send(Request::login());
            }
            None => {
                debug!("sending login request to new network.");

                let mut network = NetworkHandler::new(settings, (*notify).clone());
                network.send(screeps_rs_network::Request::login());
                state.network = Some(network);
                state.pending_since = Some(time::now_utc());
            }
        }
    }
}
