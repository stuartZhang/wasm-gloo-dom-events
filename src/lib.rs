use ::futures::{channel::mpsc, stream::Stream};
use ::gloo::{events::EventListener, history::HistoryListener, render::AnimationFrame, timers::callback::{Interval, Timeout}};
use ::std::{cell::RefCell, pin::Pin, rc::Rc, task::{Context, Poll}};
use ::web_sys::{Event as Event2, CustomEvent};

enum Listener {
    Event(EventListener),
    History(HistoryListener),
    Render(AnimationFrame),
    Interval(Interval),
    Timeout(Timeout)
}
pub enum Event {
    Event(Event2),
    CustomEvent(CustomEvent),
    None(String)
}
pub enum Vm {
    Browser(CustomEvent),
    Nodejs(String)
}
pub struct EventStream {
    sender: Rc<RefCell<mpsc::UnboundedSender<Event>>>,
    receiver: mpsc::UnboundedReceiver<Event>,
    _listener: Listener,
}
impl Stream for EventStream {
    type Item = Event;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}
#[path ="./browser-dom-element.rs"]
mod browser_dom_element;
pub use browser_dom_element::Options;
#[path ="./browser-history.rs"]
mod browser_history;
#[path ="./browser-animation-frame.rs"]
mod browser_animation_frame;
mod interval;
mod timeout;
