// impl Trait
#![feature(conservative_impl_trait)]
// Graphics
extern crate glium;
extern crate glutin;
extern crate rusttype;
#[macro_use]
extern crate conrod;
#[macro_use]
extern crate conrod_derive;
// Network
extern crate screeps_api;
extern crate scrs_network;
// Caching
extern crate time;
// Logging
extern crate chrono;
#[macro_use]
extern crate log;
extern crate fern;

pub mod app;
pub mod setup;
pub mod events;
pub mod window_loop;
pub mod ui;
pub mod network;

pub use app::App;

pub fn main<T, I>(verbose_logging: bool, debug_modules: I)
    where T: AsRef<str>,
          I: IntoIterator<Item = T>
{
    setup::init_logger(verbose_logging, debug_modules);

    let (events_loop, app) = setup::init_window();

    window_loop::main_window_loop(events_loop, app);
}
