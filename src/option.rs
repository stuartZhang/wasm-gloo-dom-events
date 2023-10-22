use ::gloo::events::EventListenerOptions;
#[derive(Default)]
pub struct Options {
    event_listener_options: EventListenerOptions,
    is_serial: bool
}
impl Options {
    pub fn enable_prevent_default(is_serial: bool) -> Self {
        Options {
            event_listener_options: EventListenerOptions::enable_prevent_default(),
            is_serial
        }
    }
    pub fn is_serial(&self) -> bool {
        self.is_serial
    }
    pub fn event_listener_options(&self) -> EventListenerOptions {
        self.event_listener_options
    }
}