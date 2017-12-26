use std::cell::Ref;
use std::collections::HashMap;

use conrod::{widget, Rect};
use conrod::render::{Primitive, PrimitiveKind};
use screeps_api::RoomName;
use screeps_api::endpoints::room_terrain::{TerrainGrid, TerrainType};
use screeps_api::websocket::RoomMapViewUpdate;
use screeps_api::websocket::types::room::objects::KnownRoomObject;

use screeps_rs_network::{MapCacheData, SelectedRooms};

use super::constants::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MapViewOffset {
    x_offset: f64,
    y_offset: f64,
    room_size: f64,
}

impl MapViewOffset {
    #[inline(always)]
    pub fn new(x: f64, y: f64, size: f64) -> Self {
        MapViewOffset {
            x_offset: x,
            y_offset: y,
            room_size: size,
        }
    }
}

// Special note here: TODO for performance!
//
// Right now, we're doing two levels of dynamic calls for ever rendered square.
// Once to our Box<Iterator<Item = Primitive<'static>>, and once to our ConstructedViewIterator
// (dynamic only through matching an enum).
//
// We could significantly speed this up if we have instead of a 'get primitive walker' method,
// a 'render with newly created primitive walker' method, all the way down the chain. Then it'd be
// map_view.rs which calls the renderer, and it would be specialized and optimizable specifically
// for each render type.

/// Logic put into separate function because it's useful both when creating an iterator
/// and when advancing that iterator.
#[inline(always)]
fn find_next_available_room<'a>(
    data: &Ref<'a, MapCacheData>,
    start_room_name: RoomName,
    mut current_relative_room_x: i16,
    mut current_relative_room_y: i16,
    horizontal_room_count: i16,
    vertical_room_count: i16,
) -> Option<(i16, i16, Ref<'a, TerrainGrid>)> {
    loop {
        // advance room name.
        if current_relative_room_x < horizontal_room_count {
            current_relative_room_x += 1;
        } else if current_relative_room_y < vertical_room_count {
            current_relative_room_x = 0;
            current_relative_room_y += 1;
        } else {
            break None;
        }

        let new_room = start_room_name
            + (
                current_relative_room_x as i32,
                current_relative_room_y as i32,
            );

        if data.terrain.contains_key(&new_room) {
            break Some((
                current_relative_room_x,
                current_relative_room_y,
                Ref::map(Ref::clone(data), |data| {
                    &data.terrain.get(&new_room).unwrap().1
                }),
            ));
        }
    }
}

/// Logic put into separate function because it's useful both when creating an iterator
/// and when advancing that iterator.
#[inline(always)]
fn find_next_available_map_view_room<'a>(
    data: &Ref<'a, MapCacheData>,
    start_room_name: RoomName,
    mut current_relative_room_x: i16,
    mut current_relative_room_y: i16,
    horizontal_room_count: i16,
    vertical_room_count: i16,
) -> Option<(i16, i16, Ref<'a, RoomMapViewUpdate>, MapIteratorState)> {
    loop {
        let room_info = loop {
            // advance room name.
            if current_relative_room_x < horizontal_room_count {
                current_relative_room_x += 1;
            } else if current_relative_room_y < vertical_room_count {
                current_relative_room_x = 0;
                current_relative_room_y += 1;
            } else {
                break None;
            }

            let new_room = start_room_name
                + (
                    current_relative_room_x as i32,
                    current_relative_room_y as i32,
                );

            if data.map_views.contains_key(&new_room) {
                debug!("found a map view for room {}", new_room);
                break Some(Ref::map(Ref::clone(data), |data| {
                    &data.map_views.get(&new_room).unwrap().1
                }));
            }
        };
        let view = match room_info {
            Some(v) => v,
            None => break None,
        };

        let inner_state = MapIteratorState::first(&view);

        if let Some(data) = inner_state {
            break Some((current_relative_room_x, current_relative_room_y, view, data));
        }
    }
}

enum MapIteratorState {
    Roads(usize),
    Power(usize),
    Walls(usize),
    Portals(usize),
    Sources(usize),
    Minerals(usize),
    Controllers(usize),
    KeeperLairs(usize),
    UserObjects(usize, usize),
}

impl MapIteratorState {
    #[inline(always)]
    fn first(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.roads.is_empty() {
            Some(MapIteratorState::Roads(0))
        } else {
            Self::next_from_roads(view)
        }
    }
    #[inline(always)]
    fn next_from_roads(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.power_or_power_bank.is_empty() {
            Some(MapIteratorState::Power(0))
        } else {
            Self::next_from_power_banks(view)
        }
    }
    #[inline(always)]
    fn next_from_power_banks(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.walls.is_empty() {
            Some(MapIteratorState::Walls(0))
        } else {
            Self::next_from_walls(view)
        }
    }
    #[inline(always)]
    fn next_from_walls(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.portals.is_empty() {
            Some(MapIteratorState::Portals(0))
        } else {
            Self::next_from_portals(view)
        }
    }
    #[inline(always)]
    fn next_from_portals(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.sources.is_empty() {
            Some(MapIteratorState::Sources(0))
        } else {
            Self::next_from_sources(view)
        }
    }
    #[inline(always)]
    fn next_from_sources(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.minerals.is_empty() {
            Some(MapIteratorState::Minerals(0))
        } else {
            Self::next_from_minerals(view)
        }
    }
    #[inline(always)]
    fn next_from_minerals(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.controllers.is_empty() {
            Some(MapIteratorState::Controllers(0))
        } else {
            Self::next_from_controllers(view)
        }
    }
    #[inline(always)]
    fn next_from_controllers(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.keeper_lairs.is_empty() {
            Some(MapIteratorState::KeeperLairs(0))
        } else {
            Self::next_from_keeper_lairs(view)
        }
    }
    #[inline(always)]
    fn next_from_keeper_lairs(view: &RoomMapViewUpdate) -> Option<MapIteratorState> {
        if !view.users_objects.is_empty() {
            let mut value = None;
            for (idx, &(_, ref data)) in view.users_objects.iter().enumerate() {
                if !data.is_empty() {
                    value = Some(MapIteratorState::UserObjects(idx, 0));
                    break;
                }
            }
            value
        } else {
            None
        }
    }
}

/// Logic put into separate function because it's useful both when creating an iterator
/// and when advancing that iterator.
#[inline(always)]
fn find_next_detail_view_room<'a>(
    data: &Ref<'a, MapCacheData>,
    start_room_name: RoomName,
    horizontal_room_count: i16,
    vertical_room_count: i16,
) -> Option<(i16, i16, Ref<'a, HashMap<String, KnownRoomObject>>)> {
    let selected_room = data.detail_view.as_ref().map(|x| x.0);

    match selected_room {
        Some(selected_room_name) => {
            let diff = selected_room_name - start_room_name;
            let x_diff = diff.0 as i16;
            let y_diff = diff.1 as i16;
            if x_diff < 0 || x_diff > horizontal_room_count || y_diff < 0 || y_diff > vertical_room_count {
                None // cut out out-of-bounds rooms.
            } else {
                Some((
                    x_diff,
                    y_diff,
                    Ref::map(Ref::clone(data), |data| {
                        &data.detail_view.as_ref().unwrap().1
                    }),
                ))
            }
        }
        None => None,
    }
}
struct ConstructedViewIterator<'a> {
    data: Ref<'a, MapCacheData>,
    render_id: widget::Id,
    render_scizzor: Rect,
    start_room_name: RoomName,
    horizontal_room_count: i16,
    vertical_room_count: i16,
    start_room_screen_pos: (f64, f64),
    room_screen_size: f64,
    state: ConstructedViewIteratorState<'a>,
}

enum ConstructedViewIteratorState<'a> {
    TerrainRender {
        current_room_terrain: Ref<'a, TerrainGrid>,
        current_relative_room_x: i16,
        current_relative_room_y: i16,
        current_terrain_square: (u8, u8),
    },
    MapViewRender {
        current_room_view: Ref<'a, RoomMapViewUpdate>,
        current_relative_room_x: i16,
        current_relative_room_y: i16,
        inner_state: MapIteratorState,
    },
    RoomRender {
        current_room: Ref<'a, HashMap<String, KnownRoomObject>>,
        current_relative_room_x: i16,
        current_relative_room_y: i16,
    },
    Done,
}

impl<'a> Iterator for ConstructedViewIterator<'a> {
    type Item = Primitive<'static>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let mut next_state = None;

        let value = match self.state {
            ConstructedViewIteratorState::TerrainRender {
                ref mut current_relative_room_x,
                ref mut current_relative_room_y,
                ref mut current_room_terrain,
                current_terrain_square: (ref mut current_terrain_x, ref mut current_terrain_y),
            } => {
                // Render terrain square
                let primitive = {
                    let terrain_type = current_room_terrain[*current_terrain_y as usize][*current_terrain_x as usize];

                    let terrain_square_length = self.room_screen_size / 50.0;

                    let x_pos = self.start_room_screen_pos.0
                        + self.room_screen_size
                            * (*current_relative_room_x as f64 + ((*current_terrain_x as f64) / 50.0));
                    let y_pos = self.start_room_screen_pos.1
                        + self.room_screen_size
                            * (*current_relative_room_y as f64 + (((50 - *current_terrain_y) as f64) / 50.0));

                    Primitive {
                        id: self.render_id,
                        kind: PrimitiveKind::Rectangle {
                            color: match terrain_type {
                                TerrainType::Plains => PLAINS_COLOR,
                                TerrainType::Swamp => SWAMP_COLOR,
                                TerrainType::SwampyWall | TerrainType::Wall => WALL_COLOR,
                            },
                        },
                        scizzor: self.render_scizzor,
                        rect: Rect::from_corners(
                            [x_pos, y_pos - terrain_square_length],
                            [x_pos + terrain_square_length, y_pos],
                        ),
                    }
                };

                // advance terrain x/y
                if *current_terrain_x < 49 {
                    *current_terrain_x += 1;
                } else if *current_terrain_y < 49 {
                    *current_terrain_x = 0;
                    *current_terrain_y += 1;
                    assert_eq!(current_room_terrain[*current_terrain_y as usize].len(), 50);
                } else {
                    // advance room name
                    let possibly_found = find_next_available_room(
                        &self.data,
                        self.start_room_name,
                        *current_relative_room_x,
                        *current_relative_room_y,
                        self.horizontal_room_count,
                        self.vertical_room_count,
                    );

                    match possibly_found {
                        Some((new_relative_room_x, new_relative_room_y, new_view)) => {
                            *current_terrain_x = 0;
                            *current_terrain_y = 0;
                            *current_relative_room_x = new_relative_room_x;
                            *current_relative_room_y = new_relative_room_y;
                            *current_room_terrain = new_view;
                        }
                        None => match find_next_available_map_view_room(
                            &self.data,
                            self.start_room_name,
                            -1,
                            0,
                            self.horizontal_room_count,
                            self.vertical_room_count,
                        ) {
                            Some((new_relative_room_x, new_relative_room_y, new_view, new_state)) => {
                                next_state = Some(ConstructedViewIteratorState::MapViewRender {
                                    current_room_view: new_view,
                                    current_relative_room_x: new_relative_room_x,
                                    current_relative_room_y: new_relative_room_y,
                                    inner_state: new_state,
                                });
                            }
                            None => {
                                next_state = Some(ConstructedViewIteratorState::Done);
                            }
                        },
                    }
                }

                Some(primitive)
            }
            ConstructedViewIteratorState::MapViewRender {
                ref mut current_room_view,
                ref mut current_relative_room_x,
                ref mut current_relative_room_y,
                ref mut inner_state,
            } => {
                let draw_square_at = {
                    let room_screen_size = self.room_screen_size;
                    let start_room_screen_pos = self.start_room_screen_pos;
                    let render_id = self.render_id;
                    let render_scizzor = self.render_scizzor;
                    let current_relative_room_x = *current_relative_room_x;
                    let current_relative_room_y = *current_relative_room_y;
                    move |in_room_x, in_room_y, color| {
                        let terrain_square_length = room_screen_size / 50.0;

                        let x_pos = start_room_screen_pos.0
                            + room_screen_size * (current_relative_room_x as f64 + (((in_room_x as f64) - 1.0) / 50.0))
                            + terrain_square_length;
                        let y_pos = start_room_screen_pos.1
                            + room_screen_size * (current_relative_room_y as f64 + ((49.0 - (in_room_y as f64)) / 50.0))
                            + terrain_square_length;

                        let visual_length = terrain_square_length;

                        Primitive {
                            id: render_id,
                            kind: PrimitiveKind::Rectangle { color: color },
                            scizzor: render_scizzor,
                            rect: Rect::from_corners(
                                [x_pos, y_pos - visual_length],
                                [x_pos + visual_length, y_pos],
                            ),
                        }
                    }
                };

                let value;

                let next = match *inner_state {
                    MapIteratorState::Roads(idx) => {
                        let (x_pos, y_pos) = current_room_view.roads[idx];

                        value = draw_square_at(x_pos, y_pos, ROAD_COLOR);

                        if idx + 1 < current_room_view.roads.len() {
                            Some(MapIteratorState::Roads(idx + 1))
                        } else {
                            MapIteratorState::next_from_roads(&current_room_view)
                        }
                    }
                    MapIteratorState::Power(idx) => {
                        let (x_pos, y_pos) = current_room_view.power_or_power_bank[idx];

                        value = draw_square_at(x_pos, y_pos, POWER_COLOR);

                        if idx + 1 < current_room_view.power_or_power_bank.len() {
                            Some(MapIteratorState::Power(idx + 1))
                        } else {
                            MapIteratorState::next_from_power_banks(&current_room_view)
                        }
                    }
                    MapIteratorState::Walls(idx) => {
                        let (x_pos, y_pos) = current_room_view.walls[idx];

                        // TODO: differentiate between natural walls and these walls.
                        value = draw_square_at(x_pos, y_pos, WALL_COLOR);

                        if idx + 1 < current_room_view.walls.len() {
                            Some(MapIteratorState::Walls(idx + 1))
                        } else {
                            MapIteratorState::next_from_walls(&current_room_view)
                        }
                    }
                    MapIteratorState::Portals(idx) => {
                        let (x_pos, y_pos) = current_room_view.portals[idx];

                        // TODO: differentiate between natural walls and these walls.
                        value = draw_square_at(x_pos, y_pos, PORTAL_COLOR);

                        if idx + 1 < current_room_view.portals.len() {
                            Some(MapIteratorState::Portals(idx + 1))
                        } else {
                            MapIteratorState::next_from_portals(&current_room_view)
                        }
                    }
                    MapIteratorState::Sources(idx) => {
                        let (x_pos, y_pos) = current_room_view.sources[idx];

                        value = draw_square_at(x_pos, y_pos, SOURCE_COLOR);

                        if idx + 1 < current_room_view.sources.len() {
                            Some(MapIteratorState::Sources(idx + 1))
                        } else {
                            MapIteratorState::next_from_sources(&current_room_view)
                        }
                    }
                    MapIteratorState::Minerals(idx) => {
                        let (x_pos, y_pos) = current_room_view.minerals[idx];

                        value = draw_square_at(x_pos, y_pos, MINERAL_COLOR);

                        if idx + 1 < current_room_view.minerals.len() {
                            Some(MapIteratorState::Minerals(idx + 1))
                        } else {
                            MapIteratorState::next_from_minerals(&current_room_view)
                        }
                    }
                    MapIteratorState::Controllers(idx) => {
                        let (x_pos, y_pos) = current_room_view.controllers[idx];

                        value = draw_square_at(x_pos, y_pos, CONTROLLER_COLOR);

                        if idx + 1 < current_room_view.controllers.len() {
                            Some(MapIteratorState::Controllers(idx + 1))
                        } else {
                            MapIteratorState::next_from_controllers(&current_room_view)
                        }
                    }
                    MapIteratorState::KeeperLairs(idx) => {
                        let (x_pos, y_pos) = current_room_view.keeper_lairs[idx];

                        value = draw_square_at(x_pos, y_pos, KEEPER_COLOR);

                        if idx + 1 < current_room_view.keeper_lairs.len() {
                            Some(MapIteratorState::KeeperLairs(idx + 1))
                        } else {
                            MapIteratorState::next_from_keeper_lairs(&current_room_view)
                        }
                    }
                    MapIteratorState::UserObjects(user_idx, obj_idx) => {
                        // TODO: differentiate between friendly and hostile users.
                        let objs = &current_room_view.users_objects[user_idx].1;
                        let (x_pos, y_pos) = objs[obj_idx];

                        value = draw_square_at(x_pos, y_pos, USER_COLOR);

                        if obj_idx + 1 < objs.len() {
                            Some(MapIteratorState::UserObjects(user_idx, obj_idx + 1))
                        } else {
                            let mut next = None;
                            let to_test = &current_room_view.users_objects;
                            for (test_user_idx, &(_, ref objs)) in to_test.iter().enumerate().skip(user_idx + 1) {
                                if !objs.is_empty() {
                                    next = Some(MapIteratorState::UserObjects(test_user_idx, 0))
                                }
                            }
                            next
                        }
                    }
                };

                match next {
                    Some(state) => *inner_state = state,
                    None => {
                        next_state = match find_next_available_map_view_room(
                            &self.data,
                            self.start_room_name,
                            *current_relative_room_x,
                            *current_relative_room_y,
                            self.horizontal_room_count,
                            self.vertical_room_count,
                        ) {
                            Some((new_relative_room_x, new_relative_room_y, new_view, new_state)) => {
                                Some(ConstructedViewIteratorState::MapViewRender {
                                    current_room_view: new_view,
                                    current_relative_room_x: new_relative_room_x,
                                    current_relative_room_y: new_relative_room_y,
                                    inner_state: new_state,
                                })
                            }
                            None => match find_next_detail_view_room(
                                &self.data,
                                self.start_room_name,
                                self.horizontal_room_count,
                                self.vertical_room_count,
                            ) {
                                Some((new_relative_room_x, new_relative_room_y, new_view)) => {
                                    Some(ConstructedViewIteratorState::RoomRender {
                                        current_room: new_view,
                                        current_relative_room_x: new_relative_room_x,
                                        current_relative_room_y: new_relative_room_y,
                                    })
                                }
                                None => Some(ConstructedViewIteratorState::Done),
                            },
                        };
                    }
                }

                Some(value)
            }
            ConstructedViewIteratorState::RoomRender {
                ref mut current_relative_room_x,
                ref mut current_relative_room_y,
                current_room: ref mut _current_room,
            } => {
                // TODO: this is a placeholder to make sure we're getting clicking on the screen right.
                next_state = Some(ConstructedViewIteratorState::Done);
                let room_screen_size = self.room_screen_size;
                let start_room_screen_pos = self.start_room_screen_pos;
                let render_id = self.render_id;
                let render_scizzor = self.render_scizzor;
                let current_relative_room_x = *current_relative_room_x;
                let current_relative_room_y = *current_relative_room_y;
                let terrain_square_length = room_screen_size / 50.0;

                let x_pos = start_room_screen_pos.0
                    + room_screen_size * (current_relative_room_x as f64 + (((0.0 as f64) - 1.0) / 50.0))
                    + terrain_square_length;
                let y_pos = start_room_screen_pos.1
                    + room_screen_size * (current_relative_room_y as f64 + ((49.0 - (0.0 as f64)) / 50.0))
                    + terrain_square_length;

                let visual_length = terrain_square_length * 50.0;

                Some(Primitive {
                    id: render_id,
                    kind: PrimitiveKind::Rectangle {
                        color: KEEPER_COLOR,
                    },
                    scizzor: render_scizzor,
                    rect: Rect::from_corners(
                        [x_pos, y_pos - visual_length],
                        [x_pos + visual_length, y_pos],
                    ),
                })
            }
            ConstructedViewIteratorState::Done => None,
        };

        if let Some(state) = next_state {
            self.state = state;
        }

        value
    }
}

#[inline(always)]
pub fn render<'a>(
    id: widget::Id,
    view_rect: Rect,
    scizzor: Rect,
    parameters: (SelectedRooms, Ref<'a, MapCacheData>, MapViewOffset),
) -> impl Iterator<Item = Primitive<'static>> + 'a {
    let (selected, data, offset) = parameters;

    let start_room_name = selected.start;
    let horizontal_room_count = (selected.end.x_coord - selected.start.x_coord) as i16;
    let vertical_room_count = (selected.end.y_coord - selected.start.y_coord) as i16;

    let room_square_initial_screen_x = view_rect.x.start + offset.x_offset;
    let room_square_initial_screen_y = view_rect.y.start + offset.y_offset;
    let room_square_screen_edge_length = offset.room_size;

    let state = match find_next_available_room(
        &data,
        start_room_name,
        -1,
        0,
        horizontal_room_count,
        vertical_room_count,
    ) {
        Some((new_relative_room_x, new_relative_room_y, new_room_view)) => {
            ConstructedViewIteratorState::TerrainRender {
                current_room_terrain: new_room_view,
                current_relative_room_x: new_relative_room_x,
                current_relative_room_y: new_relative_room_y,
                current_terrain_square: (0, 0),
            }
        }
        None => match find_next_available_map_view_room(
            &data,
            start_room_name,
            -1,
            0,
            horizontal_room_count,
            vertical_room_count,
        ) {
            Some((new_relative_room_x, new_relative_room_y, new_map_view, new_state)) => {
                ConstructedViewIteratorState::MapViewRender {
                    current_room_view: new_map_view,
                    current_relative_room_x: new_relative_room_x,
                    current_relative_room_y: new_relative_room_y,
                    inner_state: new_state,
                }
            }
            None => match find_next_detail_view_room(
                &data,
                start_room_name,
                horizontal_room_count,
                vertical_room_count,
            ) {
                Some((new_relative_room_x, new_relative_room_y, new_view)) => {
                    ConstructedViewIteratorState::RoomRender {
                        current_room: new_view,
                        current_relative_room_x: new_relative_room_x,
                        current_relative_room_y: new_relative_room_y,
                    }
                }
                None => ConstructedViewIteratorState::Done,
            },
        },
    };

    ConstructedViewIterator {
        data: data,
        render_id: id,
        render_scizzor: scizzor,
        start_room_name: start_room_name,
        horizontal_room_count: horizontal_room_count,
        vertical_room_count: vertical_room_count,
        start_room_screen_pos: (room_square_initial_screen_x, room_square_initial_screen_y),
        room_screen_size: room_square_screen_edge_length,
        state: state,
    }
}
