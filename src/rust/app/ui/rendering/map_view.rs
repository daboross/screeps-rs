use std::cell::Ref;

use conrod::{widget, Rect};
use conrod::render::{Primitive, PrimitiveKind};
use screeps_api::RoomName;
use screeps_api::endpoints::room_terrain::{TerrainGrid, TerrainType};

use network::{SelectedRooms, MapCacheData};

use super::constants::{PLAINS_COLOR, WALL_COLOR, SWAMP_COLOR};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MapViewOffset {
    x_offset: f64,
    y_offset: f64,
    room_size: f64,
}

impl MapViewOffset {
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
fn find_next_available_room<'a>(data: &Ref<'a, MapCacheData>,
                                start_room_name: RoomName,
                                mut current_relative_room_x: i16,
                                mut current_relative_room_y: i16,
                                horizontal_room_count: i16,
                                vertical_room_count: i16)
                                -> Option<(i16, i16, Ref<'a, TerrainGrid>)> {
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

        let new_room = start_room_name + (current_relative_room_x as i32, current_relative_room_y as i32);

        if data.terrain.contains_key(&new_room) {
            break Some((current_relative_room_x,
                        current_relative_room_y,
                        Ref::map(Ref::clone(data),
                                 |data| &data.terrain.get(&new_room).unwrap().1)));
        }
    }
}

enum ConstructedViewIterator<'a> {
    TerrainRender {
        data: Ref<'a, MapCacheData>,
        current_room_terrain: Ref<'a, TerrainGrid>,
        render_id: widget::Id,
        render_scizzor: Rect,
        start_room_name: RoomName,
        horizontal_room_count: i16,
        vertical_room_count: i16,
        current_relative_room_x: i16,
        current_relative_room_y: i16,
        room_square_initial_screen_x: f64,
        room_square_initial_screen_y: f64,
        room_square_screen_edge_length: f64,
        current_terrain_square: (u8, u8),
    },
    Done,
}

impl<'a> Iterator for ConstructedViewIterator<'a> {
    type Item = Primitive<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut next_state = None;

        let value = match *self {
            ConstructedViewIterator::TerrainRender {
                ref data,
                ref mut current_room_terrain,
                render_id,
                render_scizzor,
                ref start_room_name,
                horizontal_room_count,
                vertical_room_count,
                ref mut current_relative_room_x,
                ref mut current_relative_room_y,
                room_square_initial_screen_x,
                room_square_initial_screen_y,
                room_square_screen_edge_length,
                current_terrain_square: (ref mut current_terrain_x, ref mut current_terrain_y)
            } => {
                // Render terrain square
                let primitive = {
                    let terrain_type = current_room_terrain[*current_terrain_y as usize][*current_terrain_x as usize];

                    let terrain_square_length = room_square_screen_edge_length / 50.0;

                    let x_pos = room_square_initial_screen_x +
                                room_square_screen_edge_length *
                                (*current_relative_room_x as f64 + ((*current_terrain_x as f64) / 50.0));
                    let y_pos = room_square_initial_screen_y +
                                room_square_screen_edge_length *
                                (*current_relative_room_y as f64 + ((*current_terrain_y as f64) / 50.0));

                    Primitive {
                        id: render_id,
                        kind: PrimitiveKind::Rectangle {
                            color: match terrain_type {
                                TerrainType::Plains => PLAINS_COLOR,
                                TerrainType::Swamp => SWAMP_COLOR,
                                TerrainType::SwampyWall | TerrainType::Wall => WALL_COLOR,
                            },
                        },
                        scizzor: render_scizzor,
                        rect: Rect::from_corners([x_pos, y_pos],
                                                 [x_pos + terrain_square_length, y_pos + terrain_square_length]),
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
                        data,
                        *start_room_name,
                        *current_relative_room_x,
                        *current_relative_room_y,
                        horizontal_room_count,
                        vertical_room_count
                    );

                    match possibly_found {
                        Some((new_relative_room_x, new_relative_room_y, new_view)) => {
                            *current_terrain_x = 0;
                            *current_terrain_y = 0;
                            *current_relative_room_x = new_relative_room_x;
                            *current_relative_room_y = new_relative_room_y;
                            *current_room_terrain = new_view;
                        }
                        None => {
                            next_state = Some(ConstructedViewIterator::Done);
                        }
                    }
                }

                Some(primitive)
            }
            ConstructedViewIterator::Done => None,
        };

        if let Some(state) = next_state {
            *self = state;
        }

        value
    }
}

pub fn render<'a>(id: widget::Id,
                  view_rect: Rect,
                  scizzor: Rect,
                  parameters: (SelectedRooms, Ref<'a, MapCacheData>, MapViewOffset))
                  -> impl Iterator<Item = Primitive<'static>> + 'a {

    let (selected, data, offset) = parameters;

    let start_room_name = selected.start;
    let horizontal_room_count = (selected.end.x_coord - selected.start.x_coord) as i16;
    let vertical_room_count = (selected.end.y_coord - selected.start.y_coord) as i16;

    match find_next_available_room(&data,
                                   start_room_name,
                                   -1,
                                   0,
                                   horizontal_room_count,
                                   vertical_room_count) {
        Some((new_relative_room_x, new_relative_room_y, new_room_view)) => {
            let room_square_initial_screen_x = view_rect.x.start + offset.x_offset;
            let room_square_initial_screen_y = view_rect.y.start + offset.y_offset;
            let room_square_screen_edge_length = offset.room_size;

            ConstructedViewIterator::TerrainRender {
                data: data,
                current_room_terrain: new_room_view,
                render_id: id,
                render_scizzor: scizzor,
                start_room_name: start_room_name,
                horizontal_room_count: horizontal_room_count,
                vertical_room_count: vertical_room_count,
                current_relative_room_x: new_relative_room_x,
                current_relative_room_y: new_relative_room_y,
                room_square_initial_screen_x: room_square_initial_screen_x,
                room_square_initial_screen_y: room_square_initial_screen_y,
                room_square_screen_edge_length: room_square_screen_edge_length,
                current_terrain_square: (0, 0),
            }
        }
        None => ConstructedViewIterator::Done,
    }
}
