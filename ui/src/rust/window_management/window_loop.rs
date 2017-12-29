use glium::Surface;
use app::{App, AppCell};
use super::glutin_glue::{Event, EventLoop};

use {conrod, glium, glium_backend, glutin, layout, rendering, ui_state};

pub fn main_window_loop(events: glutin::EventsLoop, mut app: App) {
    let mut events = EventLoop::new(events);

    let mut state = ui_state::State::new();

    debug!("Starting event loop.");

    events.run_loop(|control, event| {
        if let ui_state::ScreenState::Exit = state.screen_state {
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
                                input:
                                    glutin::KeyboardInput {
                                        virtual_keycode: Some(glutin::VirtualKeyCode::Escape),
                                        ..
                                    },
                                ..
                            }
                            | glutin::WindowEvent::Closed => control.exit(),
                            glutin::WindowEvent::Refresh | glutin::WindowEvent::Resized(..) => {
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
                    let App {
                        ref mut ui,
                        ref display,
                        ref mut image_cache,
                        ref mut ids,
                        ref mut renderer,
                        ref mut net_cache,
                        ref mut network_handler,
                        ref notify,
                        ..
                    } = app;

                    let mut ui_cell = ui.set_widgets();

                    let mut cell = AppCell::cell(
                        &mut ui_cell,
                        display,
                        image_cache,
                        ids,
                        renderer,
                        net_cache,
                        network_handler,
                        &mut additional_render,
                        notify,
                    );

                    // Create main screen.
                    layout::create_ui(&mut cell, &mut state);
                }

                let ready_additional_render = additional_render.map(|r| {
                    r.ready(&app.ui)
                        .expect("expected custom render widget to exist")
                });
                // Render the `Ui` and then display it on the screen.
                if let Some(mut primitives) = app.ui.draw_if_changed() {
                    use rendering::RenderPipelineFinish;
                    use layout::BACKGROUND_RGB;

                    if let Some(renderer) = ready_additional_render.as_ref() {
                        renderer.prepare_images(&app.display, &mut app.image_cache);
                    }

                    struct RenderFinish<'a> {
                        display: &'a glium::Display,
                        image_map: &'a conrod::image::Map<rendering::render_cache::Texture>,
                        renderer: &'a mut glium_backend::Renderer,
                    }
                    impl<'a> RenderPipelineFinish for RenderFinish<'a> {
                        fn render_with<T>(self, walker: T)
                        where
                            T: conrod::render::PrimitiveWalker,
                        {
                            let RenderFinish {
                                display,
                                image_map,
                                renderer,
                            } = self;

                            renderer.fill(&*display, walker, &*image_map);
                        }
                    }
                    {
                        let last_step = RenderFinish {
                            display: &app.display,
                            image_map: &app.image_cache.image_map,
                            renderer: &mut app.renderer,
                        };

                        match ready_additional_render {
                            Some(mut r) => r.render_with(primitives, &app.image_cache, last_step),
                            None => last_step.render_with(primitives),
                        }
                    }

                    let mut target = app.display.draw();
                    target.clear_color(BACKGROUND_RGB[0], BACKGROUND_RGB[1], BACKGROUND_RGB[2], 1.0);
                    app.renderer
                        .draw(&app.display, &mut target, &app.image_cache.image_map)
                        .expect("expected drawing GUI to display to succeed");
                    target
                        .finish()
                        .expect("expected frame to remain unfinished at this point in the main loop.");
                }
            }
        }
    });
}
