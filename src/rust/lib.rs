// Graphics
#[macro_use]
extern crate conrod;
extern crate glium;
extern crate glutin;
extern crate rusttype;
// Network
extern crate hyper;
extern crate hyper_rustls;
extern crate screeps_api;
extern crate fern;
// Logging
#[macro_use]
extern crate log;
extern crate time;

pub mod debugging;
pub mod ui;
pub mod network;
pub mod glue;

use debugging::{FailureUnwrap, FailStage};
use glium::{DisplayBuild, Surface};
pub use glue::App;
use glue::AppCell;
use ui::Event;


pub fn main(verbose_logging: bool) {
    debugging::setup_logger(verbose_logging);

    // Create window.
    let display = glutin::WindowBuilder::new()
        .with_dimensions(640, 480)
        .with_vsync()
        .with_title("Screeps Conrod Client")
        .build_glium()
        .uw(FailStage::Startup, "Failed to create glutin window.");

    // Create UI and other components.
    let mut app = App::new(display);

    // Add font.
    app.ui.fonts.insert(akashi_font());

    main_window_loop(app);
}

fn main_window_loop(mut app: App) {
    let mut events = ui::EventLoop::new(&app.display);

    let mut state = ui::GraphicsState::login_screen();

    debug!("[lib]\tStarting event loop.");

    loop {
        if let ui::GraphicsState::Exit = state {
            info!("exiting.");
            break;
        }

        let next_event = events.next();

        match next_event {
            Event::Glutin(event) => {
                debug!("[lib]\tGlutin Event: {:?}", event);

                // Use the `winit` backend feature to convert the winit event to a conrod one.
                if let Some(event) = conrod::backend::winit::convert(event.clone(), &app.display) {
                    debug!("[lib]\tConrod Event: {:?}", event);

                    app.ui.handle_event(event);
                    events.needs_update();
                }

                match event {
                    // Break from the loop upon `Escape`.
                    glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                    glutin::Event::Closed => return,
                    // glutin::Event::Focused(true) |
                    glutin::Event::Refresh | glutin::Event::Awakened => {
                        app.ui.needs_redraw();
                        events.needs_update();
                    }
                    _ => (),
                }
            }
            Event::UpdateUi => {
                debug!("[lib]\tUpdateUI Event.");

                {
                    let App { ref mut ui,
                              ref display,
                              ref mut image_map,
                              ref mut ids,
                              ref mut renderer,
                              ref mut net_cache,
                              .. } = app;

                    let mut ui_cell = ui.set_widgets();
                    let mut cell = AppCell::cell(&mut ui_cell, display, image_map, ids, renderer, net_cache);

                    // Create main screen.
                    ui::create(&mut cell, &mut state);
                }

                // Render the `Ui` and then display it on the screen.
                if let Some(primitives) = app.ui.draw_if_changed() {

                    app.renderer.fill(&app.display, primitives, &app.image_map);
                    let mut target = app.display.draw();
                    target.clear_color(0.0, 0.0, 0.0, 1.0);
                    app.renderer
                        .draw(&app.display, &mut target, &app.image_map)
                        .uw(FailStage::Runtime, "Failed to draw target to display");
                    target.finish().uw(FailStage::Runtime, "Failed to finish target drawing");
                }
            }
            Event::None => {
                error!("Warning: could not find any events.");
            }
        }
    }
}

fn akashi_font() -> rusttype::Font<'static> {
    let font_data = include_bytes!("../ttf/Akashi.ttf");
    let collection = rusttype::FontCollection::from_bytes(font_data as &[u8]);

    collection.into_font().uw(FailStage::Startup,
                              "Failed to load built in Akashi.ttf font.")
}
