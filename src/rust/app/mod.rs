pub mod events;
pub mod ui;

pub use self::events::{EventLoop, Event};
pub use self::ui::{GraphicsState, create_ui};

use std::marker::PhantomData;

use conrod;
use glium;

use debugging::{FailureUnwrap, FailureUnwrapDebug, FailStage};
use network::NetCache;

pub struct App {
    pub ui: conrod::Ui,
    pub display: glium::Display,
    pub image_map: conrod::image::Map<glium::texture::Texture2d>,
    pub ids: ui::Ids,
    pub renderer: conrod::backend::glium::Renderer,
    pub net_cache: NetCache,
    /// Phantom data in order to allow adding any additional fields in the future.
    #[doc(hidden)]
    pub _phantom: PhantomData<()>,
}

pub struct AppCell<'a, 'b: 'a, 'c> {
    pub ui: &'a mut conrod::UiCell<'b>,
    pub display: &'a glium::Display,
    pub image_map: &'a mut conrod::image::Map<glium::texture::Texture2d>,
    pub ids: &'a mut ui::Ids,
    pub renderer: &'a mut conrod::backend::glium::Renderer,
    pub net_cache: &'a mut NetCache,
    pub additional_rendering: &'c mut Option<ui::AdditionalRender>,
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
            ids: ids,
            renderer: renderer,
            net_cache: NetCache::new(),
            _phantom: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, 'c> AppCell<'a, 'b, 'c> {
    pub fn cell(cell: &'a mut conrod::UiCell<'b>,
                display: &'a glium::Display,
                image_map: &'a mut conrod::image::Map<glium::texture::Texture2d>,
                ids: &'a mut ui::Ids,
                renderer: &'a mut conrod::backend::glium::Renderer,
                net_cache: &'a mut NetCache,
                additional_rendering: &'c mut Option<ui::AdditionalRender>)
                -> Self {

        AppCell {
            ui: cell,
            display: display,
            image_map: image_map,
            ids: ids,
            renderer: renderer,
            net_cache: net_cache,
            additional_rendering: additional_rendering,
            _phantom: PhantomData,
        }
    }
}
