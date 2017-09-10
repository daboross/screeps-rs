// impl Trait
#![feature(conservative_impl_trait)]
// Graphics

#[macro_use]
extern crate conrod;
#[macro_use]
extern crate conrod_derive;
extern crate glium;
extern crate glutin;
extern crate rusttype;
// Network

extern crate screeps_api;
extern crate screeps_rs_network;
// Caching

extern crate time;
// Logging

extern crate chrono;
extern crate fern;
#[macro_use]
extern crate log;

pub mod app;
pub mod layout;
pub mod ui_state;
pub mod rendering;
pub mod network_integration;
pub mod window_management;
pub mod widgets;
mod map_view_utils;

pub use app::App;
pub use network_integration::NetworkHandler;

pub fn main<T, I>(verbose_logging: bool, debug_modules: I)
where
    T: AsRef<str>,
    I: IntoIterator<Item = T>,
{
    window_management::setup::init_logger(verbose_logging, debug_modules);

    let (events_loop, app) = window_management::setup::init_window();

    window_management::window_loop::main_window_loop(events_loop, app);
}
