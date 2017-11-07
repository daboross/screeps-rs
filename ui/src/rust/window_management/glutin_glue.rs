use std::{thread, time};

use glutin;

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
pub struct EventLoop {
    events_loop: glutin::EventsLoop,
    last_ui_update: time::Instant,
}

pub struct LoopControl {
    ui_needs_update: u8,
    exiting: bool,
}

impl LoopControl {
    /// Notifies the event loop that the `Ui` requires another update whether or not there are any
    /// pending events.
    ///
    /// This is primarily used on the occasion that some part of the `Ui` is still animating and
    /// requires further updates to do so.
    pub fn needs_update(&mut self) {
        self.ui_needs_update = 3;
    }

    /// Notifies the loop to skip all current pending events, and exit the loop immediately afterwards.
    pub fn exit(&mut self) {
        self.exiting = true;
    }
}

impl EventLoop {
    pub fn new(events_loop: glutin::EventsLoop) -> Self {
        EventLoop {
            events_loop: events_loop,
            last_ui_update: time::Instant::now() - time::Duration::from_millis(16),
        }
    }

    fn poll_events<F>(&mut self, callback: F)
    where
        F: FnMut(glutin::Event),
    {
        self.events_loop.poll_events(callback);
    }

    fn wait_events<F>(&mut self, mut callback: F)
    where
        F: FnMut(glutin::Event),
    {
        self.events_loop.run_forever(|event| {
            callback(event);

            glutin::ControlFlow::Break
        })
    }

    /// Gets the next event. If there are no glutin events available, this either returns `Event::UpdateUi` or waits
    /// for an event depending on if the UI needs updating
    pub fn run_loop<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut LoopControl, Event),
    {
        let mut control = LoopControl {
            ui_needs_update: 3,
            exiting: false,
        };

        loop {
            self.poll_events(|evt| {
                if !control.exiting {
                    callback(&mut control, Event::Glutin(evt))
                }
            });

            if control.exiting {
                break;
            }

            if control.ui_needs_update > 0 {
                let sixteen_ms = time::Duration::from_millis(16);
                let time_since = self.last_ui_update.elapsed();
                if time_since < sixteen_ms {
                    thread::sleep(sixteen_ms - time_since);
                    // re-poll for window events once more, then when this code block runs sixteen milliseconds
                    // will definitely have passed.
                    continue;
                }
                self.last_ui_update = time::Instant::now();
                control.ui_needs_update -= 1;
                callback(&mut control, Event::UpdateUi);
                continue;
            }

            self.wait_events(|evt| {
                if !control.exiting {
                    callback(&mut control, Event::Glutin(evt))
                }
            });

            if control.exiting {
                break;
            }
        }
    }
}

/// Event returned from EventLoop.
pub enum Event {
    /// A glutin event was found.
    Glutin(glutin::Event),
    /// The UI needs to be updated.
    UpdateUi,
}
