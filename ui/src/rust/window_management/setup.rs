use std::io;

pub use app::App;

use {chrono, fern, glium, glutin, log, rusttype};

fn akashi_font() -> rusttype::Font<'static> {
    let font_data = include_bytes!("../../ttf/Akashi.ttf");
    let collection = rusttype::FontCollection::from_bytes(font_data as &[u8]);

    collection
        .into_font()
        .expect("expected loading embedded Akashi.ttf font to succeed")
}

pub fn init_window() -> (glutin::EventsLoop, App) {
    // Create window.
    let events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_dimensions(640, 480)
        .with_title("screeps-rs-client");
    let context = glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_multisampling(4);
    let display =
        glium::Display::new(window, context, &events_loop).expect("expected initial window creation to succeed");

    // Create UI and other components.
    let mut app = App::new(display, &events_loop);

    // Add font.
    app.ui.fonts.insert(akashi_font());

    (events_loop, app)
}

pub fn init_logger<T, I>(verbose: bool, debug_modules: I)
where
    T: AsRef<str>,
    I: IntoIterator<Item = T>,
{
    let mut dispatch = fern::Dispatch::new()
        .level(if verbose {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        })
        .level_for("rustls", log::LevelFilter::Warn)
        .level_for("hyper", log::LevelFilter::Warn);

    for module in debug_modules {
        dispatch = dispatch.level_for(module.as_ref().to_owned(), log::LevelFilter::Trace);
    }

    dispatch
        .format(|out, msg, record| {
            let now = chrono::Local::now();

            out.finish(format_args!(
                "[{}][{}] {}: {}",
                now.format("%H:%M:%S"),
                record.level(),
                record.target(),
                msg
            ));
        })
        .chain(io::stdout())
        .apply()
        .unwrap_or_else(|_| warn!("Logging initialization failed: a global logger was already set!"));
}
