use conrod;
use debugging::{FailureUnwrap, FailureUnwrapDebug, FailStage};
use glium;

use std::marker::PhantomData;
use ui;

pub struct App {
    pub ui: conrod::Ui,
    pub display: glium::Display,
    pub image_map: conrod::image::Map<glium::texture::Texture2d>,
    pub ui_ids: ui::Ids,
    pub renderer: conrod::backend::glium::Renderer,
    /// TODO: Cache of stuff retrieved from the server.
    pub net_cache: (),
    /// Phantom data in order to allow adding any additional fields in the future.
    #[doc(hidden)]
    pub _phantom: PhantomData<()>,
}

impl App {
    pub fn new(window: glium::Display) -> Self {
        let (width, height) = window.get_window()
            .uw(FailStage::Startup, "Failed to get window.")
            .get_inner_size()
            .uw(FailStage::Startup, "Failed to get window size.");

        // Create UI.
        let mut ui = conrod::UiBuilder::new([width as f64, height as f64]).build();
        let renderer = conrod::backend::glium::Renderer::new(&window)
            .uwd(FailStage::Startup, "Failed to load conrod glium renderer.");
        let image_map = conrod::image::Map::new();
        let ids = ui::Ids::new(ui.widget_id_generator());

        App {
            ui: ui,
            display: window,
            image_map: image_map,
            ui_ids: ids,
            renderer: renderer,
            net_cache: (),
            _phantom: PhantomData,
        }
    }
}
