use ::gloo::events::EventListenerOptions;
#[derive(Default)]
pub struct Options {
    event_listener_options: EventListenerOptions,
    is_serial: bool
}
impl Options {
    /// 形参`is_serial: bool`表示：当事件被频繁且连续地被触发时，事件处理函数是否被允许并发地执行。
    /// * is_serial = true  串行执行
    /// * is_serial = false 并发执行
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
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