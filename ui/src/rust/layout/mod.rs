mod login_screen;
mod room_view;
mod left_panel;

use std::default::Default;
use std::mem;

use conrod::{self, color, Borderable, Colorable, Widget};
use conrod::widget::*;
use conrod::widget::id;

use app::AppCell;
use screeps_rs_network;
use rendering::AdditionalRender;

pub use self::login_screen::LoginScreenState;
pub use self::room_view::RoomViewState;

const HEADER_HEIGHT: conrod::Scalar = 30.0;

pub const BACKGROUND_RGB: [f32; 3] = [0.0625, 0.46875, 0.3125];
pub const BACKGROUND: conrod::Color = conrod::Color::Rgba(BACKGROUND_RGB[0], BACKGROUND_RGB[1], BACKGROUND_RGB[2], 1.0);

#[derive(Debug)]
pub enum GraphicsState {
    LoginScreen(LoginScreenState),
    RoomView(RoomViewState),
    Exit,
}

impl GraphicsState {
    pub fn login_screen() -> Self {
        GraphicsState::LoginScreen(LoginScreenState::default())
    }
}

pub fn create_ui(app: &mut AppCell, state: &mut GraphicsState) {
    let mut update = None;

    let result = match *state {
        GraphicsState::LoginScreen(ref mut inner) => {
            login_screen::create_ui(app, inner, &mut update);
            Ok(())
        }
        GraphicsState::RoomView(ref mut inner) => room_view::create_ui(app, inner, &mut update),
        GraphicsState::Exit => panic!("Should have exited."),
    };

    if let Some(inner) = update {
        *state = inner;
    }

    match result {
        Ok(()) => (),
        Err(screeps_rs_network::NotLoggedIn) => {
            let mut temp_state = GraphicsState::login_screen();
            // leave the UI in a reasonable state if we error out in the next few lines before re-swapping.
            mem::swap(state, &mut temp_state);
            let new_state = match temp_state {
                GraphicsState::LoginScreen(state) => GraphicsState::LoginScreen(state),
                GraphicsState::RoomView(state) => {
                    GraphicsState::LoginScreen(LoginScreenState::new(state.into_network()))
                }
                GraphicsState::Exit => GraphicsState::Exit,
            };
            *state = new_state;
        }
    }
}

fn frame(ui: &mut conrod::UiCell, ids: &Ids, body_id: Id, body: Canvas) {
    let header = Canvas::new()
        .color(color::DARK_CHARCOAL)
        .border(0.0)
        .length(HEADER_HEIGHT);

    Canvas::new()
        .color(BACKGROUND)
        .border(0.0)
        .flow_down(&[(ids.root.header, header), (body_id, body)])
        .set(ids.root.root, ui);
}

pub struct RootIds {
    root: Id,
    header: Id,
    body: Id,
}
impl RootIds {
    pub fn new(gen: &mut id::Generator) -> Self {
        RootIds {
            root: gen.next(),
            header: gen.next(),
            body: gen.next(),
        }
    }
}

pub struct Ids {
    root: RootIds,
    left_panel: left_panel::LeftPanelIds,
    login: login_screen::LoginIds,
    room_view: room_view::RoomViewIds,
}

impl Ids {
    pub fn new(gen: &mut id::Generator) -> Self {
        Ids {
            root: RootIds::new(gen),
            left_panel: left_panel::LeftPanelIds::new(gen),
            login: login_screen::LoginIds::new(gen),
            room_view: room_view::RoomViewIds::new(gen),
        }
    }
}
