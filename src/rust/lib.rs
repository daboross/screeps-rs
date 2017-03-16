#[macro_use]
extern crate conrod;
extern crate glium;
extern crate glutin;
extern crate rusttype;

pub mod failure;
pub mod ui;

// Brings in `.uw()` and `.uwd()` to scope for prettier panics (these functions are similar to .unwrap()).

use failure::{FailureUnwrap, FailureUnwrapDebug, FailStage};
use glium::{DisplayBuild, Surface};
use ui::Event;


pub fn main() {
    // Create window.
    let display = glium::glutin::WindowBuilder::new()
        .with_vsync()
        .with_title("Screeps Conrod Client")
        .build_glium()
        .uw(FailStage::Startup, "Failed to create glutin window.");

    let (width, height) = display.get_window()
        .uw(FailStage::Startup, "Failed to get window.")
        .get_inner_size()
        .uw(FailStage::Startup, "Failed to get window size.");

    // Create UI.
    let mut ui = conrod::UiBuilder::new([width as f64, height as f64]).build();

    // Add font.
    ui.fonts.insert(akashi_font());

    // A type used for converting `conrod::render::Primitives` into `Command`s that can be used
    // for drawing to the glium `Surface`.
    let renderer = conrod::backend::glium::Renderer::new(&display)
        .uwd(FailStage::Startup, "Failed to load conrod glium renderer.");

    // The image map describing each of our widget->image mappings (in our case, none).
    let image_map = conrod::image::Map::new();

    // Instantiate the generated list of widget identifiers.
    let ids = ui::Ids::new(ui.widget_id_generator());


    main_window_loop(display, ui, image_map, ids, renderer)
}

fn main_window_loop(display: glium::backend::glutin_backend::GlutinFacade,
                    mut ui: conrod::Ui,
                    image_map: conrod::image::Map<glium::texture::Texture2d>,
                    mut ids: ui::Ids,
                    mut renderer: conrod::backend::glium::Renderer) {
    let mut events = ui::EventLoop::new(&display);

    loop {
        let next_event = events.next();

        match next_event {
            Event::Glutin(event) => {
                // Use the `winit` backend feature to convert the winit event to a conrod one.
                if let Some(event) = conrod::backend::winit::convert(event.clone(), &display) {
                    println!("[lib]\tConrod Event: {:?}", event);
                    ui.handle_event(event);
                    events.needs_update();
                }
                println!("[lib]\tGlutin Event: {:?}", event);
                match event {
                    // Break from the loop upon `Escape`.
                    glutin::Event::KeyboardInput(_, _, Some(glium::glutin::VirtualKeyCode::Escape)) |
                    glutin::Event::Closed => return,
                    // glutin::Event::Focused(true) |
                    glutin::Event::Refresh => {
                        ui.needs_redraw();
                        events.needs_update();
                    }
                    _ => (),
                }
            }
            Event::UpdateUi => {
                println!("[lib]\tUpdateUI Event.");
                {
                    let mut ui_cell = ui.set_widgets();

                    // Create main screen.
                    ui::main_screen(&mut ui_cell, &mut ids);
                }
                // Render the `Ui` and then display it on the screen.
                if let Some(primitives) = ui.draw_if_changed() {
                    println!("[lib]\tRedraw.");
                    renderer.fill(&display, primitives, &image_map);
                    let mut target = display.draw();
                    target.clear_color(0.0, 0.0, 0.0, 1.0);
                    renderer.draw(&display, &mut target, &image_map)
                        .uwd(FailStage::Runtime, "Failed to draw target to display");
                    target.finish().uw(FailStage::Runtime, "Failed to finish target drawing");
                }
            }
            Event::None => {
                println!("Warning: could not find any events.");
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
