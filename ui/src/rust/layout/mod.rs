mod login_screen;
mod room_view;
mod left_panel;

use std::collections::VecDeque;

use conrod::{self, color, Borderable, Colorable, Widget};
use conrod::widget::*;
use conrod::widget::id;

use app::AppCell;
use rendering::AdditionalRender;
use ui_state::{ScreenState, State};

const HEADER_HEIGHT: conrod::Scalar = 30.0;

pub const BACKGROUND_RGB: [f32; 3] = [0.0625, 0.46875, 0.3125];
pub const BACKGROUND: conrod::Color = conrod::Color::Rgba(BACKGROUND_RGB[0], BACKGROUND_RGB[1], BACKGROUND_RGB[2], 1.0);

pub fn create_ui(app: &mut AppCell, state: &mut State) {
    let mut update = VecDeque::new();

    match state.screen_state {
        ScreenState::Login(ref login_state) => {
            login_screen::create_ui(app, login_state, &mut update);
        }
        ScreenState::Map(ref map_state) => {
            room_view::create_ui(app, map_state, &mut update);
        }
        ScreenState::Exit => {}
    }

    state.transform(update.drain(..));
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
