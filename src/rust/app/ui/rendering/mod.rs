use std::marker::PhantomData;
use std::rc::Rc;

use conrod::{widget, Rect, Color};
use conrod::render::{PrimitiveWalker, Primitive, PrimitiveKind};
use screeps_api::RoomName;
use screeps_api::endpoints::room_terrain::{TerrainGrid, TerrainType};

const WALL_COLOR: Color = Color::Rgba(0.07, 0.07, 0.07, 1.0);
const SWAMP_COLOR: Color = Color::Rgba(0.16, 0.17, 0.09, 1.0);
const PLAINS_COLOR: Color = Color::Rgba(0.17, 0.17, 0.17, 1.0);

#[derive(Clone, Debug)]
pub enum AdditionalRenderType {
    Room(RoomName, Rc<TerrainGrid>),
    Rooms(Vec<(Rect, Rc<TerrainGrid>)>),
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

struct RcTerrainIterator {
    x: u8,
    y: u8,
    inner: Rc<TerrainGrid>,
}

impl RcTerrainIterator {
    fn new(terrain: Rc<TerrainGrid>) -> Self {
        assert_eq!(terrain.len(), 50);

        assert_eq!(terrain[0].len(), 50);

        RcTerrainIterator {
            x: 0,
            y: 0,
            inner: terrain,
        }
    }
}

impl Iterator for RcTerrainIterator {
    /// (x, y, type)
    type Item = (u8, u8, TerrainType);

    fn next(&mut self) -> Option<Self::Item> {
        if self.y == 50 {
            return None;
        }
        let item = (self.x, self.y, self.inner[self.y as usize][self.x as usize]);

        if self.x < 49 {
            self.x += 1;
        } else {
            self.x = 0;
            self.y += 1;
            if self.y < 50 {
                assert_eq!(self.inner[self.y as usize].len(), 50);
            }
        }

        Some(item)
    }
}

fn render_terrain(id: widget::Id,
                  rect: Rect,
                  scizzor: Rect,
                  terrain: Rc<TerrainGrid>)
                  -> impl Iterator<Item = Primitive<'static>> {
    debug!("rendering room at {:?}", rect);
    let (width, height) = rect.w_h();
    let size_unit = f64::min(width / 50.0, height / 50.0);
    // conrod has rectangles by default constructed from the center position not the lower left...
    let left_edge = rect.left();
    let bottom_edge = rect.bottom();

    RcTerrainIterator::new(terrain).map(move |(x, y, tile)| {
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
}

impl AdditionalRender {
    pub fn room(replace: widget::Id, name: RoomName, terrain: Rc<TerrainGrid>) -> Self {
        AdditionalRender {
            replace: replace,
            draw_type: AdditionalRenderType::Room(name, terrain),
            _phantom: PhantomData,
        }
    }

    pub fn room_grid(replace: widget::Id, rooms: Vec<(Rect, Rc<TerrainGrid>)>) -> Self {
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
