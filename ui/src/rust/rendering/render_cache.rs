use std::collections::hash_map::{Entry, HashMap};

use screeps_api::{RoomName, TerrainGrid};
use conrod::image::{Id as ImageId, Map as ImageMap};
use glium;

use super::map_view;

// pub type Texture = glium::texture::CompressedSrgbTexture2d;
pub type Texture = glium::texture::SrgbTexture2d;

pub struct RenderCache {
    room_terrains: HashMap<RoomName, ImageId>,
    pub image_map: ImageMap<Texture>,
}

impl RenderCache {
    pub fn new() -> Self {
        RenderCache {
            room_terrains: HashMap::new(),
            image_map: ImageMap::new(),
        }
    }

    /// Invalidates a generated terrain image, so the next time it's fetched it must be
    /// updated.
    pub fn invalidate_terrain(&mut self, room_name: RoomName) {
        if let Some(id) = self.room_terrains.remove(&room_name) {
            self.image_map.remove(id);
        }
    }

    /// Updates the terrain *now* for a room and stores the new updated terrain image.
    pub fn update_terrain(&mut self, display: &glium::Display, room_name: RoomName, terrain: &TerrainGrid) {
        let new_texture = Texture::new(display, map_view::make_terrain_texture(terrain))
            .expect("expected creating srgb texture to suceed");

        match self.room_terrains.entry(room_name) {
            Entry::Occupied(entry) => {
                self.image_map.replace(*entry.get(), new_texture);
            }
            Entry::Vacant(entry) => {
                entry.insert(self.image_map.insert(new_texture));
            }
        }
    }

    /// Gets a generated image for the given room's terrain, or generates it from the given
    /// terrain grid if it doesn't exist yet.
    pub fn get_or_generate_terrain(
        &mut self,
        display: &glium::Display,
        room_name: RoomName,
        terrain: &TerrainGrid,
    ) -> ImageId {
        let RenderCache {
            ref mut room_terrains,
            ref mut image_map,
        } = *self;

        room_terrains
            .entry(room_name)
            .or_insert_with(|| {
                let new_texture = Texture::new(display, map_view::make_terrain_texture(terrain))
                    .expect("expected creating srgb texture to suceed");

                image_map.insert(new_texture)
            })
            .clone()
    }

    /// Gets already generated terrain image for the given room name. If it doesn't exist yet,
    /// or was invalidated, returns `None`.
    pub fn get_terrain(&self, room_name: RoomName) -> Option<ImageId> {
        self.room_terrains.get(&room_name).cloned()
    }

    /// Removes all cached rendered images which aren't within the two RoomNames, inclusively.
    pub fn invalidate_outside_of(&mut self, r1: RoomName, r2: RoomName) {
        let RenderCache {
            ref mut room_terrains,
            ref mut image_map,
        } = *self;

        let min_x = r1.x_coord.min(r1.x_coord);
        let max_x = r1.x_coord.max(r2.x_coord);
        let min_y = r1.y_coord.min(r2.y_coord);
        let max_y = r1.y_coord.max(r2.y_coord);

        room_terrains.retain(|key, value| {
            if key.x_coord >= min_x && key.x_coord <= max_x && key.y_coord >= min_y && key.y_coord <= max_y {
                true
            } else {
                image_map.remove(*value);
                false
            }
        });
    }
}
