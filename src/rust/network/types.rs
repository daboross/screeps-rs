use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use time;
use screeps_api::{self, RoomName};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct SelectedRooms {
    pub start: RoomName,
    pub end: RoomName,
    pub focused_room: Option<RoomName>,
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
            focused_room: None,
        }
    }

    #[inline]
    pub fn contains(&self, room: &RoomName) -> bool {
        (self.start.x_coord < room.x_coord && room.x_coord < self.end.x_coord) &&
        (self.start.y_coord < room.y_coord && room.y_coord < self.end.y_coord)
    }

    #[inline]
    pub fn set_focus<T: Into<Option<RoomName>>>(&mut self, room: T) {
        self.focused_room = room.into();
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

#[derive(Default, Debug)]
pub struct MapCacheData {
    // TODO: should we be re-fetching terrain at some point, or is it alright to leave it forever in memory?
    // The client can always restart to clear this.
    pub terrain: HashMap<RoomName, (time::Timespec, screeps_api::TerrainGrid)>,
    /// Map views, the Timespec is when the data was fetched.
    pub map_views: HashMap<RoomName, (time::Timespec, screeps_api::websocket::RoomMapViewUpdate)>,
}

pub type MapCache = Rc<RefCell<MapCacheData>>;
