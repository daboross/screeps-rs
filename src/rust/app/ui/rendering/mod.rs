use std::marker::PhantomData;

use conrod::{widget, Rect};
use conrod::render::{PrimitiveWalker, Primitive, PrimitiveKind};
use screeps_api::RoomName;
use screeps_api::endpoints::room_terrain::{TerrainGrid, TerrainType};

#[derive(Clone, Debug)]
pub enum AdditionalRenderType {
    Room(RoomName, TerrainGrid),
}

#[derive(Clone, Debug)]
pub struct AdditionalRender {
    pub replace: widget::Id,
    pub draw_type: AdditionalRenderType,
    _phantom: PhantomData<()>,
}

pub struct MergedPrimitives<T: Sized> {
    custom: Option<AdditionalRender>,
    currently_replacing: Option<Box<Iterator<Item = Primitive<'static>>>>,
    walker: T,
}

impl AdditionalRender {
    pub fn room(replace: widget::Id, name: RoomName, terrain: TerrainGrid) -> Self {
        AdditionalRender {
            replace: replace,
            draw_type: AdditionalRenderType::Room(name, terrain),
            _phantom: PhantomData,
        }
    }

    pub fn merged_walker<T: PrimitiveWalker>(self, walker: T) -> MergedPrimitives<T> {
        MergedPrimitives {
            custom: Some(self),
            currently_replacing: None,
            walker: walker,
        }
    }

    pub fn into_primitives(self, replacing_primitive: Primitive) -> Box<Iterator<Item = Primitive<'static>>> {
        let parent_rect = replacing_primitive.rect;
        let parent_scizzor = replacing_primitive.scizzor;
        debug!("into_primitives: {{parent_rect: {:?}, parent_scizzor: {:?}}}",
               parent_rect,
               parent_scizzor);

        let AdditionalRender { replace, draw_type, .. } = self;

        match draw_type {
            AdditionalRenderType::Room(_, grid) => {
                let left_edge = parent_rect.left();
                let bottom_edge = parent_rect.bottom();
                let (parent_width, parent_height) = parent_rect.w_h();
                let width_unit = parent_width / 50.0;
                let height_unit = parent_height / 50.0;
                let terrain_dim = [width_unit, height_unit];

                debug!("found width_unit: {}, height_unit: {}, dimensions: {:?}",
                       width_unit,
                       height_unit,
                       terrain_dim);

                Box::new(grid.into_iter().enumerate().flat_map(move |(y, row)| {
                    debug!("primitive row {}", y);
                    row.into_iter().enumerate().map(move |(x, tile)| {
                        use conrod::color::*;

                        Primitive {
                            id: replace,
                            kind: PrimitiveKind::Rectangle {
                                color: match tile {
                                    TerrainType::Plains => LIGHT_GREY,
                                    TerrainType::Wall => DARK_BROWN,
                                    TerrainType::SwampyWall => DARK_GREEN,
                                    TerrainType::Swamp => LIGHT_GREEN,
                                },
                            },
                            scizzor: parent_scizzor,
                            rect: Rect::from_xy_dim([left_edge + (x as f64) * width_unit,
                                                     bottom_edge + (y as f64) * height_unit],
                                                    terrain_dim),
                        }
                    })
                }))
            }
        }
    }
}

impl<T: PrimitiveWalker> PrimitiveWalker for MergedPrimitives<T> {
    fn next_primitive(&mut self) -> Option<Primitive> {
        if let Some(ref mut iter) = self.currently_replacing {
            if let Some(p) = iter.next() {
                return Some(p);
            }
        }
        if self.currently_replacing.is_some() {
            self.currently_replacing = None;
        }

        match self.walker.next_primitive() {
            Some(p) => {
                if Some(&p.id) == self.custom.as_ref().map(|c| &c.replace) {
                    debug!("found correct id");
                    let c = self.custom.clone().unwrap();

                    let mut iter = c.into_primitives(p);
                    let first = iter.next().expect("expected at least one primitive");
                    self.currently_replacing = Some(iter);

                    Some(first)
                } else {
                    Some(p)
                }
            }
            None => None,
        }
    }
}
