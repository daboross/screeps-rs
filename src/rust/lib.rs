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
extern crate chrono;

pub mod app;
pub mod debugging;
pub mod network;

use debugging::{FailureUnwrap, FailStage};
use glium::{DisplayBuild, Surface};
pub use app::App;
use app::{AppCell, Event};

pub fn main<T, I>(verbose_logging: bool, debug_modules: Option<I>)
    where T: AsRef<str>,
          I: IntoIterator<Item = T>
{
    debugging::setup_logger(verbose_logging, debug_modules);

    // Create window.
    let display = glutin::WindowBuilder::new()
        .with_dimensions(640, 480)
        .with_vsync()
        .with_title("SCRS Client")
        .build_glium()
        .uw(FailStage::Startup, "Error creating window.");

    // Create UI and other components.
    let mut app = App::new(display);

    // Add font.
    app.ui.fonts.insert(akashi_font());

    main_window_loop(app);
}

fn main_window_loop(mut app: App) {
    let mut events = app::EventLoop::new(&app.display);

    let mut state = app::GraphicsState::login_screen();

    debug!("Starting event loop.");

    loop {
        if let app::GraphicsState::Exit = state {
            info!("exiting.");
            break;
        }

        let next_event = events.next();

        match next_event {
            Event::Glutin(event) => {
                debug!("Glutin Event: {:?}", event);

                // Use the `winit` backend feature to convert the winit event to a conrod one.
                if let Some(event) = conrod::backend::winit::convert(event.clone(), &app.display) {
                    debug!("Conrod Event: {:?}", event);

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
                debug!("UpdateUI Event.");

                let additional_render;

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
                    app::create_ui(&mut cell, &mut state);

                    additional_render = cell.additional_rendering.take();
                }

                // Render the `Ui` and then display it on the screen.
                if let Some(primitives) = app.ui.draw_if_changed() {
                    use app::ui::BACKGROUND_RGB;

                    match additional_render {
                        Some(r) => app.renderer.fill(&app.display, r.merged_walker(primitives), &app.image_map),
                        None => app.renderer.fill(&app.display, primitives, &app.image_map),
                    }

                    let mut target = app.display.draw();
                    target.clear_color(BACKGROUND_RGB[0], BACKGROUND_RGB[1], BACKGROUND_RGB[2], 1.0);
                    app.renderer
                        .draw(&app.display, &mut target, &app.image_map)
                        .uw(FailStage::Runtime, "Error drawing GUI to display");
                    target.finish().expect("Frame shouldn't be finished yet.");
                }
            }
            Event::None => {
                error!("Empty event cycle.");
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
