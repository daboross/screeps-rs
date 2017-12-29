use std::ops::{Generator, GeneratorState};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MapViewOffset {
    pub(super) x_offset: f64,
    pub(super) y_offset: f64,
    pub(super) room_size: f64,
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

pub(super) struct IterAdapter<G>(pub(super) G);

impl<G> Iterator for IterAdapter<G>
where
    G: Generator<Return = ()>,
{
    type Item = G::Yield;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.resume() {
            GeneratorState::Yielded(item) => Some(item),
            GeneratorState::Complete(()) => None,
        }
    }
}
