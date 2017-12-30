use std::marker::PhantomData;
use std::sync::Arc;

use screeps_rs_network::{self, MemCache};

use {conrod, glium, glium_backend, glutin, layout, rendering};

use network_integration::{GlutinNotify, NetworkCache, NetworkHandler};

pub struct App {
    pub ui: conrod::Ui,
    pub display: glium::Display,
    pub image_cache: rendering::RenderCache,
    pub ids: layout::Ids,
    pub renderer: glium_backend::Renderer,
    pub net_cache: MemCache,
    pub network_handler: NetworkHandler,
    pub notify: GlutinNotify,
    /// Phantom data in order to allow adding any additional fields in the future.
    #[doc(hidden)]
    pub _phantom: PhantomData<()>,
}

pub struct AppCell<'a, 'b: 'a, 'c> {
    pub ui: &'a mut conrod::UiCell<'b>,
    pub display: &'a glium::Display,
    pub image_cache: &'a mut rendering::RenderCache,
    pub ids: &'a mut layout::Ids,
    pub renderer: &'a mut glium_backend::Renderer,
    pub net_cache: NetworkCache<'a>,
    pub additional_rendering: &'c mut Option<rendering::AdditionalRender>,
    pub notify: &'a GlutinNotify,
    /// Phantom data in order to allow adding any additional fields in the future.
    #[doc(hidden)]
    pub _phantom: PhantomData<()>,
}

impl App {
    pub fn new(window: glium::Display, events: &glutin::EventsLoop) -> Self {
        let (width, height) = window
            .gl_window()
            .window()
            .get_inner_size()
            .expect("expected getting window size to succeed.");

        // Create UI.
        let mut ui = conrod::UiBuilder::new([width as f64, height as f64]).build();
        let renderer =
            glium_backend::Renderer::new(&window).expect("expected loading conrod glium renderer to succeed.");
        let image_cache = rendering::RenderCache::new();
        let ids = layout::Ids::new(&mut ui.widget_id_generator());

        let notify = GlutinNotify::from(Arc::new(events.create_proxy()));

        App {
            ui: ui,
            display: window,
            image_cache: image_cache,
            ids: ids,
            renderer: renderer,
            net_cache: MemCache::new(),
            network_handler: NetworkHandler::new(
                screeps_rs_network::ConnectionSettings::new(String::new(), String::new(), None),
                notify.clone(),
            ),
            notify: notify,
            _phantom: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, 'c> AppCell<'a, 'b, 'c> {
    pub fn cell(
        cell: &'a mut conrod::UiCell<'b>,
        display: &'a glium::Display,
        image_cache: &'a mut rendering::RenderCache,
        ids: &'a mut layout::Ids,
        renderer: &'a mut glium_backend::Renderer,
        net_cache: &'a mut MemCache,
        network_handler: &'a mut NetworkHandler,
        additional_rendering: &'c mut Option<rendering::AdditionalRender>,
        notify: &'a GlutinNotify,
    ) -> Self {
        let net_cache = net_cache.align(
            network_handler,
            |x| {
                // TODO: this shouldn't be done here, but rather within the UI event code.
                warn!("network error occurred: {}", x);
            },
            image_cache.event_handler(),
        );
        AppCell {
            ui: cell,
            display: display,
            image_cache: image_cache,
            ids: ids,
            renderer: renderer,
            net_cache: net_cache,
            additional_rendering: additional_rendering,
            notify: notify,
            _phantom: PhantomData,
        }
    }
}
