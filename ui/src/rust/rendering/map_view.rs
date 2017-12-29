use std::cell::Ref;
use std::collections::HashMap;
use std::ops::{Generator, GeneratorState};

use conrod::{widget, Rect};
use conrod::render::{Primitive, PrimitiveKind};
use screeps_api::RoomName;
use screeps_api::endpoints::room_terrain::{TerrainGrid, TerrainType};
use screeps_api::websocket::RoomMapViewUpdate;
use screeps_api::websocket::types::room::objects::KnownRoomObject;

use screeps_rs_network::{MapCacheData, SelectedRooms};

use super::constants::*;
use super::types::{IterAdapter, MapViewOffset};

#[derive(Copy, Clone)]
struct RenderData {
    id: widget::Id,
    scizzor: Rect,
    offset: MapViewOffset,
    start_room_screen_pos: (f64, f64),
}

pub fn render<'a>(
    id: widget::Id,
    view_rect: Rect,
    scizzor: Rect,
    (selected, data, offset): (SelectedRooms, Ref<'a, MapCacheData>, MapViewOffset),
) -> impl Iterator<Item = Primitive<'static>> + 'a {
    let render_data = RenderData {
        id,
        scizzor,
        offset,
        start_room_screen_pos: (
            view_rect.x.start + offset.x_offset,
            view_rect.y.start + offset.y_offset,
        ),
    };

    let gen = move || {
        let start_room_name = selected.start;
        let horizontal_room_count = selected.end.x_coord - selected.start.x_coord;
        let vertical_room_count = selected.end.y_coord - selected.start.y_coord;

        // terrain
        for relative_room_x in 0..horizontal_room_count {
            for relative_room_y in 0..vertical_room_count {
                let current_name = start_room_name + (relative_room_x, relative_room_y);

                // if we can render this room
                if data.terrain.contains_key(&current_name) {
                    // use Ref trick to get an "owned" reference to the specific sub-bit of the data.
                    let terrain = Ref::map(Ref::clone(&data), |data| {
                        &data.terrain.get(&current_name).unwrap().1
                    });
                    yield_from!(render_terrain_of(
                        render_data,
                        relative_room_x,
                        relative_room_y,
                        terrain,
                    ));
                }
            }
        }

        // map view
        for relative_room_x in 0..horizontal_room_count {
            for relative_room_y in 0..vertical_room_count {
                let current_name = start_room_name + (relative_room_x, relative_room_y);

                // if we can render this room
                if data.map_views.contains_key(&current_name) {
                    let map_data = Ref::map(Ref::clone(&data), |data| {
                        &data.map_views.get(&current_name).unwrap().1
                    });
                    yield_from!(render_map_view_of(
                        render_data,
                        relative_room_x,
                        relative_room_y,
                        map_data,
                    ));
                }
            }
        }

        // room view
        let opt_viewed: Option<RoomName> = data.detail_view.as_ref().map(|&(name, _)| name);
        if let Some(viewed_room) = opt_viewed {
            let (x_diff, y_diff) = viewed_room - start_room_name;
            if x_diff >= 0 && x_diff <= horizontal_room_count && y_diff >= 0 && y_diff <= vertical_room_count {
                let room_objects = Ref::map(Ref::clone(&data), |data| {
                    &data.detail_view.as_ref().unwrap().1
                });

                yield_from!(render_room(render_data, x_diff, y_diff, room_objects));
            }
        }
    };

    IterAdapter(gen).fuse() // fuse required for generator safety
}

fn render_terrain_of<'a>(
    data: RenderData,
    current_relative_room_x: i32,
    current_relative_room_y: i32,
    terrain: Ref<'a, TerrainGrid>,
) -> impl Generator<Yield = Primitive<'static>, Return = ()> + 'a {
    move || {
        for current_terrain_x in 0..50 {
            for current_terrain_y in 0..50 {
                let terrain_type = terrain[current_terrain_y][current_terrain_x];

                let terrain_square_length = data.offset.room_size / 50.0;

                let x_pos = data.start_room_screen_pos.0
                    + data.offset.room_size * (current_relative_room_x as f64 + ((current_terrain_x as f64) / 50.0));
                let y_pos = data.start_room_screen_pos.1
                    + data.offset.room_size
                        * (current_relative_room_y as f64 + (((50 - current_terrain_y) as f64) / 50.0));

                yield Primitive {
                    id: data.id,
                    kind: PrimitiveKind::Rectangle {
                        color: match terrain_type {
                            TerrainType::Plains => PLAINS_COLOR,
                            TerrainType::Swamp => SWAMP_COLOR,
                            TerrainType::SwampyWall | TerrainType::Wall => WALL_COLOR,
                        },
                    },
                    scizzor: data.scizzor,
                    rect: Rect::from_corners(
                        [x_pos, y_pos - terrain_square_length],
                        [x_pos + terrain_square_length, y_pos],
                    ),
                };
            }
        }
    }
}

fn render_map_view_of<'a>(
    data: RenderData,
    current_relative_room_x: i32,
    current_relative_room_y: i32,
    map_view: Ref<'a, RoomMapViewUpdate>,
) -> impl Generator<Yield = Primitive<'static>, Return = ()> + 'a {
    move || {
        let room_screen_size = data.offset.room_size;
        let start_room_screen_pos = data.start_room_screen_pos;
        let render_id = data.id;
        let render_scizzor = data.scizzor;
        let terrain_square_length = room_screen_size / 50.0;

        macro_rules! draw_square_at {
            ($x:expr, $y:expr, $color:expr) => ({
                let x_pos = start_room_screen_pos.0
                    + room_screen_size * (current_relative_room_x as f64 + ((($x as f64) - 1.0) / 50.0))
                    + terrain_square_length;
                let y_pos = start_room_screen_pos.1
                    + room_screen_size * (current_relative_room_y as f64 + ((49.0 - ($y as f64)) / 50.0))
                    + terrain_square_length;

                let visual_length = terrain_square_length;

                Primitive {
                    id: render_id,
                    kind: PrimitiveKind::Rectangle { color: $color },
                    scizzor: render_scizzor,
                    rect: Rect::from_corners(
                        [x_pos, y_pos - visual_length],
                        [x_pos + visual_length, y_pos],
                    ),
                }

            })
        }

        let num_roads = map_view.roads.len();
        for idx in 0..num_roads {
            let (x, y) = map_view.roads[idx];
            yield draw_square_at!(x, y, ROAD_COLOR);
        }

        let num_power = map_view.power_or_power_bank.len();
        for idx in 0..num_power {
            let (x, y) = map_view.power_or_power_bank[idx];
            yield draw_square_at!(x, y, POWER_COLOR);
        }

        let num_walls = map_view.walls.len();
        for idx in 0..num_walls {
            let (x, y) = map_view.walls[idx];
            yield draw_square_at!(x, y, WALL_COLOR);
        }

        let num_portals = map_view.portals.len();
        for idx in 0..num_portals {
            let (x, y) = map_view.portals[idx];
            yield draw_square_at!(x, y, PORTAL_COLOR);
        }

        let num_sources = map_view.sources.len();
        for idx in 0..num_sources {
            let (x, y) = map_view.sources[idx];
            yield draw_square_at!(x, y, SOURCE_COLOR);
        }

        let num_minerals = map_view.minerals.len();
        for idx in 0..num_minerals {
            let (x, y) = map_view.minerals[idx];
            yield draw_square_at!(x, y, MINERAL_COLOR);
        }

        let num_controllers = map_view.controllers.len();
        for idx in 0..num_controllers {
            let (x, y) = map_view.controllers[idx];
            yield draw_square_at!(x, y, CONTROLLER_COLOR);
        }

        let num_keepers = map_view.keeper_lairs.len();
        for idx in 0..num_keepers {
            let (x, y) = map_view.keeper_lairs[idx];
            yield draw_square_at!(x, y, KEEPER_COLOR);
        }

        let num_users = map_view.users_objects.len();
        for idx in 0..num_users {
            let num_user_objects = map_view.users_objects[idx].1.len();
            for jdx in 0..num_user_objects {
                let (x, y) = map_view.users_objects[idx].1[jdx];
                yield draw_square_at!(x, y, USER_COLOR);
            }
        }
    }
}

fn render_room<'a>(
    data: RenderData,
    current_relative_room_x: i32,
    current_relative_room_y: i32,
    _room_objects: Ref<'a, HashMap<String, KnownRoomObject>>,
) -> impl Generator<Yield = Primitive<'static>, Return = ()> + 'a {
    move || {
        let x_pos = data.start_room_screen_pos.0 + data.offset.room_size * (current_relative_room_x as f64);
        let y_pos = data.start_room_screen_pos.1 + data.offset.room_size * (current_relative_room_y as f64);

        yield Primitive {
            id: data.id,
            kind: PrimitiveKind::Rectangle {
                color: KEEPER_COLOR,
            },
            scizzor: data.scizzor,
            rect: Rect::from_corners(
                [x_pos, y_pos - data.offset.room_size],
                [x_pos + data.offset.room_size, y_pos],
            ),
        };
    }
}
