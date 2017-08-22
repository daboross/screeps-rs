use std::marker::PhantomData;
use std::cell::Ref;

use conrod::widget;
use conrod::render::{Primitive, PrimitiveWalker};

use network::{MapCache, MapCacheData, SelectedRooms};

pub mod constants;
mod map_view;

pub use self::map_view::MapViewOffset;

#[derive(Clone, Debug)]
pub enum AdditionalRenderType {
    MapView((SelectedRooms, MapCache, MapViewOffset)),
}

enum BorrowedRenderType<'a> {
    MapView((SelectedRooms, Ref<'a, MapCacheData>, MapViewOffset)),
}

impl<'a> Clone for BorrowedRenderType<'a> {
    fn clone(&self) -> Self {
        match *self {
            BorrowedRenderType::MapView((rooms, ref data, offset)) => {
                BorrowedRenderType::MapView((rooms, Ref::clone(data), offset))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct AdditionalRender {
    pub replace: widget::Id,
    pub draw_type: AdditionalRenderType,
    _phantom: PhantomData<()>,
}

#[derive(Clone)]
struct BorrowedRender<'a> {
    replace: widget::Id,
    draw_type: BorrowedRenderType<'a>,
}

pub struct MergedPrimitives<'a, T: Sized> {
    custom: Option<BorrowedRender<'a>>,
    currently_replacing: Option<Box<Iterator<Item = Primitive<'static>> + 'a>>,
    walker: T,
}

impl AdditionalRender {
    #[inline(always)]
    pub fn map_view(replace: widget::Id, rooms: SelectedRooms, cache: MapCache, offset: MapViewOffset) -> Self {
        AdditionalRender {
            replace: replace,
            draw_type: AdditionalRenderType::MapView((rooms, cache, offset)),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn merged_walker<T: PrimitiveWalker>(&self, walker: T) -> MergedPrimitives<T> {
        MergedPrimitives {
            custom: Some(BorrowedRender {
                replace: self.replace,
                draw_type: match self.draw_type {
                    AdditionalRenderType::MapView((rooms, ref cache, offset)) => {
                        BorrowedRenderType::MapView((rooms, cache.borrow(), offset))
                    }
                },
            }),
            currently_replacing: None,
            walker: walker,
        }
    }
}

impl<'a> BorrowedRender<'a> {
    #[inline]
    pub fn into_primitives(self, replacing_primitive: &Primitive) -> Box<Iterator<Item = Primitive<'static>> + 'a> {
        let parent_rect = replacing_primitive.rect;
        let parent_scizzor = replacing_primitive.scizzor;

        debug!("into_primitives: {{parent_rect: {:?}, parent_scizzor: {:?}}}", parent_rect, parent_scizzor);

        let BorrowedRender {
            replace, draw_type, ..
        } = self;

        let scizzor = parent_scizzor
            .overlap(parent_rect)
            .unwrap_or(parent_scizzor);

        match draw_type {
            BorrowedRenderType::MapView(parameters) => {
                Box::new(map_view::render(replace, parent_rect, scizzor, parameters))
            }
        }
    }
}

impl<'a, T: PrimitiveWalker> PrimitiveWalker for MergedPrimitives<'a, T> {
    #[inline(always)]
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
            Some(p) => if Some(&p.id) == self.custom.as_ref().map(|c| &c.replace) {
                debug!("found correct id");
                let c = self.custom.clone().unwrap();

                let mut iter = c.into_primitives(&p);
                let first = iter.next();
                self.currently_replacing = Some(iter);

                Some(first.unwrap_or(p))
            } else {
                Some(p)
            },
            None => None,
        }
    }
}
