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
pub mod network;

use std::io;

use glium::Surface;
pub use app::App;
use app::{AppCell, Event};

pub fn main<T, I>(verbose_logging: bool, debug_modules: I)
    where T: AsRef<str>,
          I: IntoIterator<Item = T>
{
    setup_logger(verbose_logging, debug_modules);

    // Create window.
    let events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_dimensions(640, 480)
        .with_title("SCRS Client");
    let context = glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_multisampling(4);
    let display = glium::Display::new(window, context, &events_loop)
        .expect("expected initial window creation to succeed");

    // Create UI and other components.
    let mut app = App::new(display, &events_loop);

    // Add font.
    app.ui.fonts.insert(akashi_font());

    main_window_loop(events_loop, app);
}

fn main_window_loop(events: glutin::EventsLoop, mut app: App) {
    let mut events = app::EventLoop::new(events);

    let mut state = app::GraphicsState::login_screen();

    debug!("Starting event loop.");


    events.run_loop(|control, event| {
        if let app::GraphicsState::Exit = state {
            info!("exiting.");
            control.exit();
            return;
        }

        match event {
            Event::Glutin(event) => {
                debug!("Glutin Event: {:?}", event);

                // Use the `winit` backend feature to convert the winit event to a conrod one.
                if let Some(event) = conrod::backend::winit::convert_event(event.clone(), &app.display) {
                    debug!("Conrod Event: {:?}", event);

                    app.ui.handle_event(event);
                    control.needs_update();
                }

                match event {
                    glutin::Event::WindowEvent { event, .. } => {
                        match event {
                            // Break from the loop upon `Escape`.
                            glutin::WindowEvent::KeyboardInput {
                                input: glutin::KeyboardInput {
                                    virtual_keycode: Some(glutin::VirtualKeyCode::Escape),
                                    ..
                                } ,
                                ..
                            } |
                            glutin::WindowEvent::Closed => control.exit(),
                            // glutin::Event::Focused(true) |
                            glutin::WindowEvent::Refresh |
                            glutin::WindowEvent::Resized(..) => {
                                app.ui.needs_redraw();
                                control.needs_update();
                            }
                            _ => (),
                        }
                    }
                    glutin::Event::Awakened => {
                        app.ui.needs_redraw();
                        control.needs_update();
                    }
                    _ => (),
                }
            }
            Event::UpdateUi => {
                debug!("UpdateUI Event.");

                let mut additional_render = None;

                {
                    let App { ref mut ui,
                              ref display,
                              ref mut image_map,
                              ref mut ids,
                              ref mut renderer,
                              ref mut net_cache,
                              ref notify,
                              .. } = app;

                    let mut ui_cell = ui.set_widgets();

                    let mut cell = AppCell::cell(&mut ui_cell,
                                                 display,
                                                 image_map,
                                                 ids,
                                                 renderer,
                                                 net_cache,
                                                 &mut additional_render,
                                                 notify);

                    // Create main screen.
                    app::create_ui(&mut cell, &mut state);
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
                        .expect("expected drawing GUI to display to succeed");
                    target.finish().expect("expected frame to remain unfinished at this point in the main loop.");
                }
            }
        }
    });
}

fn akashi_font() -> rusttype::Font<'static> {
    let font_data = include_bytes!("../ttf/Akashi.ttf");
    let collection = rusttype::FontCollection::from_bytes(font_data as &[u8]);

    collection.into_font().expect("expected loading embedded Akashi.ttf font to succeed")
}

fn setup_logger<T, I>(verbose: bool, debug_modules: I)
    where T: AsRef<str>,
          I: IntoIterator<Item = T>
{
    let mut dispatch = fern::Dispatch::new()
        .level(if verbose {
            log::LogLevelFilter::Trace
        } else {
            log::LogLevelFilter::Info
        })
        .level_for("rustls", log::LogLevelFilter::Warn)
        .level_for("hyper", log::LogLevelFilter::Warn);

    for module in debug_modules {
        dispatch = dispatch.level_for(module.as_ref().to_owned(), log::LogLevelFilter::Trace);
    }

    dispatch.format(|out, msg, record| {
            let now = chrono::Local::now();

            out.finish(format_args!("[{}][{}] {}: {}",
                                    now.format("%H:%M:%S"),
                                    record.level(),
                                    record.target(),
                                    msg));
        })
        .chain(io::stdout())
        .apply()
        .unwrap_or_else(|_| warn!("Logging initialization failed: a global logger was already set!"));
}
