use {conrod, screeps_api, time};
use NetworkHandler;
use map_view_utils::{bound_zoom, zoom_multiplier_from_factor, ZOOM_MODIFIER};


#[derive(Debug, PartialEq)]
pub enum Event {
    LeftMenuOpened,
    LeftMenuClosed,
    SwitchShard(Option<String>),
    LoginUsername(String),
    LoginPassword(String),
    LoginSubmitted(time::Tm),
    MapPan {
        view_rect: conrod::Rect,
        event: MapPanEvent,
    },
    MapZoom {
        view_rect: conrod::Rect,
        event: MapZoomEvent,
    },
    MapClick {
        view_rect: conrod::Rect,
        event: MapClickEvent,
    },
    NowLoggedOut,
    LoggedInMapView,
    Exit,
}

#[derive(Debug)]
pub struct State {
    pub network: Option<NetworkHandler>,
    pub screen_state: ScreenState,
}

#[derive(Debug)]
pub enum ScreenState {
    Login(LoginScreenState),
    Map(MapScreenState),
    Exit,
}

#[derive(Default)] // the UI username and password boxes are empty by default.
pub struct LoginScreenState {
    pub pending_since: Option<time::Tm>,
    pub username: String,
    pub password: String,
}

impl ::std::fmt::Debug for LoginScreenState {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.debug_struct("LoginScreenState")
            .field("pending_since", &self.pending_since)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Debug)]
pub struct MapScreenState {
    pub shard: Option<String>,
    pub map_scroll: ScrollState,
    pub panels: PanelStates,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MenuState {
    Open,
    Closed,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PanelStates {
    pub left: MenuState,
}

impl Default for MenuState {
    fn default() -> Self {
        MenuState::Closed
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct MapPanEvent {
    /// The number of pixels scrolled horizontally.
    pub scrolled_map_x: f64,
    /// The number of pixels scrolled vertically.
    pub scrolled_map_y: f64,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct MapZoomEvent {
    /// The scroll change amount (unknown unit).
    pub zoom_change: f64,
    /// If zoom_change != 0.0, this is the x position the mouse was at, relative to the center of the widget.
    pub zoom_mouse_rel_x: f64,
    /// If zoom_change != 0.0, this is the y position the mouse was at, relative to the center of the widget.
    pub zoom_mouse_rel_y: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MapClickEvent {
    /// If the screen was clicked, the relative (x, y) that it was clicked at.
    pub clicked: (f64, f64),
}

#[derive(Debug)]
pub struct ScrollState {
    /// The horizontal scroll, in fractional rooms. 1 is 1 room.
    pub scroll_x: f64,
    /// The vertical scroll, in fractional rooms. 1 is 1 room.
    pub scroll_y: f64,
    /// The zoom factor, 1.0 is a room the same size as the minimum screen dimension.
    pub zoom_factor: f64,
    /// The room name currently selected.
    pub selected_room: Option<screeps_api::RoomName>,
}

impl MapScreenState {
    pub fn new() -> Self {
        // TODO: saved position? or use API to get position?
        MapScreenState {
            panels: PanelStates::default(),
            shard: None,
            map_scroll: ScrollState::default(),
        }
    }
}


impl State {
    pub fn new() -> State {
        State {
            network: None,
            screen_state: ScreenState::Login(LoginScreenState::default()),
        }
    }

    pub fn transform<T>(&mut self, events: T)
    where
        T: IntoIterator<Item = Event>,
    {
        for event in events {
            self.event(event);
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::LeftMenuOpened => match self.screen_state {
                ScreenState::Map(ref mut state) => {
                    state.panels.left = MenuState::Open;
                }
                _ => (),
            },
            Event::LeftMenuClosed => match self.screen_state {
                ScreenState::Map(ref mut state) => {
                    state.panels.left = MenuState::Closed;
                }
                _ => (),
            },
            // Event::ShardButton(new_shard) => if let ScreenState::Map(ref mut state) = self.screen_state {
            //     state.shard = new_shard;
            // },
            Event::LoginUsername(new_username) => if let ScreenState::Login(ref mut state) = self.screen_state {
                state.username = new_username;
            },
            Event::LoginPassword(new_password) => if let ScreenState::Login(ref mut state) = self.screen_state {
                state.password = new_password;
            },
            Event::LoginSubmitted(at) => if let ScreenState::Login(ref mut state) = self.screen_state {
                state.pending_since = Some(at);
            },
            Event::MapPan { view_rect, event } => if let ScreenState::Map(ref mut state) = self.screen_state {
                state.map_scroll.pan_event(view_rect, event);
            },
            Event::MapZoom { view_rect, event } => if let ScreenState::Map(ref mut state) = self.screen_state {
                state.map_scroll.zoom_event(view_rect, event);
            },
            Event::MapClick { view_rect, event } => if let ScreenState::Map(ref mut state) = self.screen_state {
                state.map_scroll.click_event(view_rect, event);
            },
            Event::NowLoggedOut => self.screen_state = ScreenState::Login(LoginScreenState::default()),
            Event::LoggedInMapView => self.screen_state = ScreenState::Map(MapScreenState::new()),
            Event::Exit => self.screen_state = ScreenState::Exit,
            Event::SwitchShard(shard) => if let ScreenState::Map(ref mut state) = self.screen_state {
                state.shard = shard;
            },
        }
    }
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

impl ScrollState {
    fn room_name_and_xy_from_rel_pos(
        &self,
        view_rect: conrod::Rect,
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

    fn pan_event(&mut self, view_rect: conrod::Rect, update: MapPanEvent) {
        let room_size_unit = view_rect.w().min(view_rect.h());

        let room_size = room_size_unit * zoom_multiplier_from_factor(self.zoom_factor);
        self.scroll_x += update.scrolled_map_x / room_size;
        self.scroll_y += update.scrolled_map_y / room_size;
    }

    fn zoom_event(&mut self, view_rect: conrod::Rect, update: MapZoomEvent) {
        let room_size_unit = view_rect.w().min(view_rect.h());

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
    }

    fn click_event(&mut self, view_rect: conrod::Rect, update: MapClickEvent) {
        let (clicked_x, clicked_y) = update.clicked;

        let (room_clicked, _) = self.room_name_and_xy_from_rel_pos(view_rect, clicked_x, clicked_y);

        info!("Clicked {}", room_clicked);

        self.selected_room = Some(room_clicked);
    }
}
