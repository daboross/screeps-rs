use std::marker::PhantomData;

use conrod::{widget, Rect, Color};
use conrod::render::{PrimitiveWalker, Primitive, PrimitiveKind};
use screeps_api::RoomName;
use screeps_api::endpoints::room_terrain::{TerrainGrid, TerrainType};

const WALL_COLOR: Color = Color::Rgba(0.07, 0.07, 0.07, 1.0);
const SWAMP_COLOR: Color = Color::Rgba(0.16, 0.17, 0.09, 1.0);
const PLAINS_COLOR: Color = Color::Rgba(0.17, 0.17, 0.17, 1.0);

#[derive(Clone, Debug)]
pub enum AdditionalRenderType {
    Room(RoomName, TerrainGrid),
    Rooms(Vec<(Rect, TerrainGrid)>),
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

fn render_terrain(id: widget::Id,
                  rect: Rect,
                  scizzor: Rect,
                  terrain: TerrainGrid)
                  -> impl Iterator<Item = Primitive<'static>> {
    debug!("rendering room at {:?}", rect);
    let (width, height) = rect.w_h();
    let size_unit = f64::min(width / 50.0, height / 50.0);
    // conrod has rectangles by default constructed from the center position not the lower left...
    let left_edge = rect.left();
    let bottom_edge = rect.bottom();

    terrain.into_iter().enumerate().flat_map(move |(y, row)| {
        row.into_iter().enumerate().map(move |(x, tile)| {
            let x_pos = left_edge + (x as f64) * size_unit;
            let y_pos = bottom_edge + (y as f64) * size_unit;

            Primitive {
                id: id,
                kind: PrimitiveKind::Rectangle {
                    color: match tile {
                        TerrainType::Plains => PLAINS_COLOR,
                        TerrainType::Wall | TerrainType::SwampyWall => WALL_COLOR,
                        TerrainType::Swamp => SWAMP_COLOR,
                    },
                },
                scizzor: scizzor,
                rect: Rect::from_corners([x_pos, y_pos], [x_pos + size_unit, y_pos + size_unit]),
            }
        })
    })
}

impl AdditionalRender {
    pub fn room(replace: widget::Id, name: RoomName, terrain: TerrainGrid) -> Self {
        AdditionalRender {
            replace: replace,
            draw_type: AdditionalRenderType::Room(name, terrain),
            _phantom: PhantomData,
        }
    }

    pub fn room_grid(replace: widget::Id, rooms: Vec<(Rect, TerrainGrid)>) -> Self {
        AdditionalRender {
            replace: replace,
            draw_type: AdditionalRenderType::Rooms(rooms),
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
            AdditionalRenderType::Room(_, grid) => Box::new(render_terrain(replace, parent_rect, parent_scizzor, grid)),
            AdditionalRenderType::Rooms(list) => {
                let scizzor = parent_scizzor.overlap(parent_rect).unwrap_or(parent_scizzor);
                Box::new(list.into_iter()
                    .flat_map(move |(rect, grid)| render_terrain(replace, rect, scizzor, grid)))
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
                    let first = iter.next().expect("expected at least one rendering primitive in list of primitives");
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
