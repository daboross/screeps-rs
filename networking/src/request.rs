use std::borrow::Cow;

use std::ops::Range;
use std::sync::Arc;
use std::fmt;

use screeps_api::RoomName;

use self::Request::*;

/// Error for not being logged in, and trying to send a query requiring authentication.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct NotLoggedIn;

/// Login username/password.
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct LoginDetails {
    inner: Arc<(String, String)>,
}

impl LoginDetails {
    /// Creates a new login detail struct.
    pub fn new(username: String, password: String) -> Self {
        LoginDetails { inner: Arc::new((username, password)) }
    }

    /// Gets the username.
    pub fn username(&self) -> &str {
        &self.inner.0
    }

    /// Gets the password.
    pub fn password(&self) -> &str {
        &self.inner.1
    }
}

impl fmt::Debug for LoginDetails {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LoginDetails")
            .field("username", &self.username())
            .field("password", &"<redacted>")
            .finish()
    }
}
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct SelectedRooms {
    pub start: RoomName,
    pub end: RoomName,
}

impl SelectedRooms {
    #[inline]
    pub fn new(rooms: Range<RoomName>) -> Self {
        use std::cmp::{min, max};

        let start = RoomName {
            x_coord: min(rooms.start.x_coord, rooms.end.x_coord),
            y_coord: min(rooms.start.y_coord, rooms.end.y_coord),
        };
        let end = RoomName {
            x_coord: max(rooms.start.x_coord, rooms.end.x_coord),
            y_coord: max(rooms.start.y_coord, rooms.end.y_coord),
        };

        SelectedRooms {
            start: start,
            end: end,
        }
    }

    #[inline]
    pub fn contains(&self, room: &RoomName) -> bool {
        (self.start.x_coord < room.x_coord && room.x_coord < self.end.x_coord) &&
        (self.start.y_coord < room.y_coord && room.y_coord < self.end.y_coord)
    }
}

impl IntoIterator for SelectedRooms {
    type Item = RoomName;
    type IntoIter = IterSelectedRooms;

    fn into_iter(mut self) -> IterSelectedRooms {
        if self.start.x_coord > self.end.x_coord {
            ::std::mem::swap(&mut self.start.x_coord, &mut self.end.x_coord);
        }
        if self.start.y_coord > self.end.y_coord {
            ::std::mem::swap(&mut self.start.x_coord, &mut self.end.x_coord);
        }

        IterSelectedRooms {
            start_x: self.start.x_coord,
            current: self.start,
            end: self.end,
        }
    }
}

pub struct IterSelectedRooms {
    start_x: i32,
    current: RoomName,
    end: RoomName,
}

impl Iterator for IterSelectedRooms {
    type Item = RoomName;

    fn next(&mut self) -> Option<RoomName> {
        match (self.current.x_coord == self.end.x_coord, self.current.y_coord == self.end.y_coord) {
            (false, _) => {
                let item = self.current;
                self.current.x_coord += 1;
                Some(item)
            }
            (true, false) => {
                let item = self.current;
                self.current.y_coord += 1;
                self.current.x_coord = self.start_x;
                Some(item)
            }
            (true, true) => None,
        }
    }
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Request {
    Login { details: LoginDetails },
    MyInfo,
    RoomTerrain { room_name: RoomName },
    SetMapSubscribes { rooms: SelectedRooms },
    SetFocusRoom { room: Option<RoomName> },
}

impl Request {
    pub fn login<'a, T, U>(username: T, password: U) -> Self
        where T: Into<Cow<'a, str>>,
              U: Into<Cow<'a, str>>
    {
        Login { details: LoginDetails::new(username.into().into_owned(), password.into().into_owned()) }
    }

    pub fn login_with_details(details: LoginDetails) -> Self {
        Login { details: details }
    }

    pub fn my_info() -> Self {
        MyInfo
    }

    pub fn room_terrain(room_name: RoomName) -> Self {
        RoomTerrain { room_name: room_name }
    }

    pub fn subscribe_map_view(rooms: SelectedRooms) -> Self {
        SetMapSubscribes { rooms: rooms }
    }

    pub fn focus_room(room_name: Option<RoomName>) -> Self {
        SetFocusRoom { room: room_name }
    }
}
