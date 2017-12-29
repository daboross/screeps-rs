use std::marker::PhantomData;
use std::cell::Ref;

use conrod::{self, widget, Rect};
use conrod::render::{Primitive, PrimitiveWalker};

use screeps_rs_network::{MapCache, MapCacheData, SelectedRooms};

#[macro_use]
mod macros;
pub mod constants;
mod map_view;
mod types;

pub use self::types::MapViewOffset;

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

pub struct ReadyRender {
    view_rect: Rect,
    scizzor: Rect,
    inner: AdditionalRender,
}

#[derive(Clone)]
struct BorrowedRender<'a> {
    replace: widget::Id,
    draw_type: BorrowedRenderType<'a>,
    view_rect: Rect,
    scizzor: Rect,
}

impl<'a> BorrowedRender<'a> {
    #[inline]
    pub fn into_primitives(self) -> impl Iterator<Item = Primitive<'static>> + 'a {
        let parent_rect = self.view_rect;
        let parent_scizzor = self.scizzor;

        debug!(
            "into_primitives: {{parent_rect: {:?}, parent_scizzor: {:?}}}",
            parent_rect, parent_scizzor
        );

        let BorrowedRender {
            replace, draw_type, ..
        } = self;

        let scizzor = parent_scizzor
            .overlap(parent_rect)
            .unwrap_or(parent_scizzor);

        match draw_type {
            BorrowedRenderType::MapView(parameters) => map_view::render(replace, parent_rect, scizzor, parameters),
        }
    }
}

pub trait RenderPipelineFinish {
    fn render_with<T: PrimitiveWalker>(self, T);
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
    pub fn ready(self, ui: &conrod::Ui) -> Option<ReadyRender> {
        Some(ReadyRender {
            view_rect: ui.rect_of(self.replace)?,
            scizzor: ui.visible_area(self.replace)?,
            inner: self,
        })
    }
}

impl ReadyRender {
    #[inline]
    pub fn render_with<T, F>(&self, walker: T, render_with: F)
    where
        T: PrimitiveWalker,
        F: RenderPipelineFinish,
    {
        let replace = self.inner.replace;

        let custom_gen = BorrowedRender {
            replace: self.inner.replace,
            draw_type: match self.inner.draw_type {
                AdditionalRenderType::MapView((rooms, ref cache, offset)) => {
                    BorrowedRenderType::MapView((rooms, cache.borrow(), offset))
                }
            },
            view_rect: self.view_rect,
            scizzor: self.scizzor,
        }.into_primitives();

        // WalkerAdapter(gen, PhantomData)
        render_with.render_with(MergedPrimitives {
            replace: replace,
            custom: Some(custom_gen),
            currently_replacing: None,
            walker: walker,
        })
    }
}

pub struct MergedPrimitives<T, U> {
    replace: widget::Id,
    custom: Option<U>,
    currently_replacing: Option<U>,
    walker: T,
}

impl<T, U> PrimitiveWalker for MergedPrimitives<T, U>
where
    T: PrimitiveWalker,
    U: Iterator<Item = Primitive<'static>>,
{
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
            Some(p) => if p.id == self.replace {
                self.currently_replacing = self.custom.take();
                match self.currently_replacing.as_mut().and_then(|c| c.next()) {
                    Some(first_replace) => Some(first_replace),
                    None => Some(p),
                }
            } else {
                Some(p)
            },
            None => None,
        }
    }
}
