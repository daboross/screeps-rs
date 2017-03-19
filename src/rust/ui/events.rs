use glium;
use glutin;
use std::{time, thread};

/// This manages the underlying support for glium + glutin.
///
/// This is roughly modeled off of the [`EventLoop`][ev] structure present in the `conrod` examples, with a number of
/// modifications in order to avoid any additional allocations.
///
/// Specifically, the 'next' method of the conrod example [`EventLoop`][ev] returns a Vec of events... which are
/// literally collected from an iterator. Instead, this struct implements Iterator itself, and iterates an enum for
/// either `glutin` events or an "update UI" event, which corresponds to returning the collected Vec in the conrod
/// example.
///
/// [ev]: https://github.com/PistonDevelopers/conrod/blob/master/examples/support/mod.rs#L367
pub struct EventLoop<'a> {
    display: &'a glium::Display,
    ui_needs_update: bool,
    last_ui_update: time::Instant,
}

impl<'a> EventLoop<'a> {
    pub fn new(window: &'a glium::Display) -> Self {
        EventLoop {
            display: window,
            ui_needs_update: true,
            last_ui_update: time::Instant::now() - time::Duration::from_millis(16),
        }
    }

    fn poll_event(&mut self) -> Option<glutin::Event> { self.display.get_window().and_then(|w| w.poll_events().next()) }

    fn wait_event(&mut self) -> Option<glutin::Event> { self.display.get_window().and_then(|w| w.wait_events().next()) }

    /// Gets the next event. If there are no glutin events available, this either returns `Event::UpdateUi` or waits
    /// for an event depending on if the UI needs updating
    pub fn next(&mut self) -> Event {
        if let Some(event) = self.poll_event() {
            return Event::Glutin(event);
        }

        if self.ui_needs_update {
            let sixteen_ms = time::Duration::from_millis(16);
            let mut now = time::Instant::now();
            let time_since_update = now - self.last_ui_update;
            if time_since_update < sixteen_ms {
                thread::sleep(sixteen_ms - time_since_update);
                if let Some(event) = self.poll_event() {
                    return Event::Glutin(event);
                }
                now = time::Instant::now();
            }
            self.last_ui_update = now;
            self.ui_needs_update = false;
            return Event::UpdateUi;
        }

        if let Some(event) = self.wait_event() {
            return Event::Glutin(event);
        }

        // Wait events should always return as long there is a window left, so this should normally never need to
        // happen.
        thread::sleep(time::Duration::from_millis(16));
        return Event::None;
    }

    /// Notifies the event loop that the `Ui` requires another update whether or not there are any
    /// pending events.
    ///
    /// This is primarily used on the occasion that some part of the `Ui` is still animating and
    /// requires further updates to do so.
    pub fn needs_update(&mut self) { self.ui_needs_update = true; }
}

/// Event returned from EventLoop.
pub enum Event {
    /// A glutin event was found.
    Glutin(glutin::Event),
    /// The UI needs to be updated.
    UpdateUi,
    /// The glutin window did not yield any events even with wait_events, and a UI update is not needed.
    None,
}
