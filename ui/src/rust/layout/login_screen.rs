use std::collections::VecDeque;

use conrod::{self, color, Borderable, Colorable, Labelable, Positionable, Sizeable, Widget};
use conrod::widget::*;

use time;

use screeps_rs_network::{self, ConnectionSettings};
use widgets::text_box::TextBox;
use ui_state::{Event as UiEvent, LoginScreenState};

use app::AppCell;
use layout::{frame, HEADER_HEIGHT};
const LOGIN_WIDTH: conrod::Scalar = 300.0;
const LOGIN_HEIGHT: conrod::Scalar = 200.0;

const LOGIN_PADDING: conrod::Scalar = 10.0;

const LOGIN_LOWER_SECTION_HEIGHT: conrod::Scalar = (LOGIN_HEIGHT - HEADER_HEIGHT) / 3.0;

#[derive(Copy, Clone)]
struct TextboxIds {
    canvas: Id,
    textbox: Id,
    label: Id,
}

#[derive(Copy, Clone)]
pub struct LoginIds {
    root: Id,
    header_canvas: Id,
    server: TextboxIds,
    username: TextboxIds,
    password: TextboxIds,
    shard: TextboxIds,
    submit_canvas: Id,
    exit_button: Id,
    submit_button: Id,
}

impl TextboxIds {
    pub fn new(gen: &mut id::Generator) -> Self {
        TextboxIds {
            canvas: gen.next(),
            textbox: gen.next(),
            label: gen.next(),
        }
    }
}

impl LoginIds {
    pub fn new(gen: &mut id::Generator) -> Self {
        LoginIds {
            root: gen.next(),
            header_canvas: gen.next(),
            server: TextboxIds::new(gen),
            username: TextboxIds::new(gen),
            password: TextboxIds::new(gen),
            shard: TextboxIds::new(gen),
            submit_canvas: gen.next(),
            exit_button: gen.next(),
            submit_button: gen.next(),
        }
    }
}

pub fn create_ui(app: &mut AppCell, state: &LoginScreenState, update: &mut VecDeque<UiEvent>) {
    if app.net_cache.login_state() == screeps_rs_network::LoginState::LoggedIn {
        update.push_front(UiEvent::LoggedInMapView);
    }

    let AppCell {
        ref mut ui,
        ref ids,
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
            (ids.login.server.canvas, bottom_template.clone()),
            (ids.login.username.canvas, bottom_template.clone()),
            (ids.login.password.canvas, bottom_template.clone()),
            (ids.login.shard.canvas, bottom_template.clone()),
            (ids.login.submit_canvas, bottom_template),
        ])
        // place
        .floating(true)
        .mid_top_of(ids.root.root)
        .down_from(ids.root.header, ui.window_dim()[1] / 4.0 - HEADER_HEIGHT)
        // set
        .set(ids.login.root, ui);

    fn textbox_field<F: FnMut(String)>(
        text: &str,
        mut update: F,
        ids: TextboxIds,
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
            .mid_right_with_margin_on(ids.canvas, 10.0)
            .set(ids.textbox, ui);

        let mut enter_pressed = false;

        for event in events.into_iter() {
            match event {
                TextBoxEvent::Update(s) => {
                    update(s);
                }
                TextBoxEvent::Enter => {
                    enter_pressed = true;
                    break;
                }
            }
        }
        enter_pressed
    }

    fn textbox_label(text: &str, ids: TextboxIds, ui: &mut conrod::UiCell) {
        Text::new(text)
            // style
            .font_size(ui.theme.font_size_small)
            .center_justify()
            .no_line_wrap()
            // position
            .mid_left_with_margin_on(ids.canvas, LOGIN_PADDING)
            .set(ids.label, ui);
    }

    textbox_label("server", ids.login.server, ui);
    textbox_label("username", ids.login.username, ui);
    textbox_label("password", ids.login.password, ui);
    textbox_label("shard", ids.login.shard, ui);

    let scalar_max = |f1_opt, f2_opt| match (f1_opt, f2_opt) {
        (Some(f1), Some(f2)) => Some(conrod::Scalar::max(f1, f2)),
        (Some(v), None) | (None, Some(v)) => Some(v),
        (None, None) => None,
    };
    let label_width = scalar_max(
        scalar_max(
            ui.w_of(ids.login.server.label),
            ui.w_of(ids.login.username.label),
        ),
        scalar_max(
            ui.w_of(ids.login.password.label),
            ui.w_of(ids.login.shard.label),
        ),
    ).unwrap_or(LOGIN_WIDTH / 2.0 - LOGIN_PADDING * 1.5);

    // Server field
    let server_enter_pressed = textbox_field(
        &state.server,
        |s| update.push_front(UiEvent::LoginServer(s)),
        ids.login.server,
        LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
        false,
        ui,
    );

    // Username field
    let username_enter_pressed = textbox_field(
        &state.username,
        |s| update.push_front(UiEvent::LoginUsername(s)),
        ids.login.username,
        LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
        false,
        ui,
    );

    // Password field
    let password_enter_pressed = textbox_field(
        &state.password,
        |s| update.push_front(UiEvent::LoginPassword(s)),
        ids.login.password,
        LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
        true,
        ui,
    );

    // Shard field
    let shard_enter_pressed = textbox_field(
        &state.shard,
        |s| update.push_front(UiEvent::LoginShard(s)),
        ids.login.shard,
        LOGIN_WIDTH - LOGIN_PADDING * 3.0 - label_width,
        false,
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
        update.push_front(UiEvent::Exit);
    } else if (submit_pressed || password_enter_pressed || username_enter_pressed || server_enter_pressed
        || shard_enter_pressed) && state.username.len() > 0 && state.password.len() > 0
    {
        use screeps_rs_network::Url;
        let server = if state.server.len() == 0 {
            ::screeps_api::DEFAULT_OFFICIAL_API_URL
                .parse()
                .expect("expected default URL to parse")
        } else {
            let result = if state.server.starts_with("http") || state.server.starts_with("https") {
                state.server.parse()
            } else {
                format!("http://{}", state.server).parse()
            }.map(|url: Url| {
                url.join("api/")
                    .expect("expected hardcoded URL segment to parse")
            });

            match result {
                Ok(url) => url,
                Err(e) => {
                    warn!("server URL invalid: {}", e);
                    return;
                }
            }
        };
        // TODO: UI option for shard.
        // let settings = ConnectionSettings::new(
        //     state.username.clone(),
        //     state.password.clone(),
        //     "shard0".to_owned(),
        // );
        let settings = ConnectionSettings::with_url(
            server,
            state.username.clone(),
            state.password.clone(),
            if state.shard.len() == 0 {
                None
            } else {
                Some(state.shard.clone())
            },
        );

        debug!("sending login request to existing network.");

        app.net_cache.update_settings(settings);
        app.net_cache.login();
        update.push_front(UiEvent::LoginSubmitted(time::now_utc()));
    }
}
