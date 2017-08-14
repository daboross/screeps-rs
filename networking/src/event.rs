use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use {screeps_api, time, websocket};

use screeps_api::RoomName;
use screeps_api::websocket::types::room::objects::KnownRoomObject;

#[derive(Default, Debug)]
pub struct MapCacheData {
    // TODO: should we be re-fetching terrain at some point, or is it alright to leave it forever in memory?
    // The client can always restart to clear this.
    pub terrain: HashMap<RoomName, (time::Timespec, screeps_api::TerrainGrid)>,
    /// Map views, the Timespec is when the data was fetched.
    pub map_views: HashMap<RoomName, (time::Timespec, screeps_api::websocket::RoomMapViewUpdate)>,
    /// Single current known view of a room.
    ///
    /// TODO: keep track of history of room's we've subscribed to in the past and what tick each data was updated.
    /// TODO: possibly keep history for each for an 'instant replay' functionality.
    /// TODO: handle unknown room objects better: given that we know they have at least an 'x' and 'y' property, we
    /// could definitely do a question mark in the UI with a drop-down for JSON properties the object has.
    pub detail_view: Option<(RoomName, HashMap<String, KnownRoomObject>)>,
}

pub type MapCache = Rc<RefCell<MapCacheData>>;

#[derive(Debug)]
pub enum NetworkEvent {
    Login {
        username: String,
        result: Result<(), screeps_api::Error>,
    },
    MyInfo {
        result: Result<screeps_api::MyInfo, screeps_api::Error>,
    },
    RoomTerrain {
        room_name: screeps_api::RoomName,
        result: Result<screeps_api::TerrainGrid, screeps_api::Error>,
    },
    WebsocketHttpError { error: screeps_api::Error },
    WebsocketError { error: websocket::WebSocketError },
    WebsocketParseError {
        error: screeps_api::websocket::parsing::ParseError,
    },
    MapView {
        room_name: screeps_api::RoomName,
        result: screeps_api::websocket::RoomMapViewUpdate,
    },
    RoomView {
        room_name: screeps_api::RoomName,
        result: screeps_api::websocket::RoomUpdate,
    },
}


impl NetworkEvent {
    pub fn error(&self) -> Option<&screeps_api::Error> {
        match *self {
            NetworkEvent::Login { ref result, .. } => result.as_ref().err(),
            NetworkEvent::MyInfo { ref result, .. } => result.as_ref().err(),
            NetworkEvent::RoomTerrain { ref result, .. } => result.as_ref().err(),
            NetworkEvent::WebsocketHttpError { ref error } => Some(error),
            NetworkEvent::MapView { .. } |
            NetworkEvent::RoomView { .. } |
            NetworkEvent::WebsocketError { .. } |
            NetworkEvent::WebsocketParseError { .. } => None,
        }
    }
}
