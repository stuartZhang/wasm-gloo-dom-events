mod option;
use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, stream::Stream, StreamExt};
use ::gloo::{events::{EventListener, EventListenerOptions}, history::{History, HistoryListener}, render::{self, AnimationFrame}};
use ::serde::{Deserialize, Serialize};
use ::std::{borrow::Cow, cell::RefCell, convert::Into, future::Future, pin::Pin, rc::Rc, task::{Context, Poll}};
use ::wasm_bindgen::prelude::*;
use ::web_sys::{Event as Event2, CustomEvent, EventTarget};
pub use option::Options;
enum Listener {
    Event(EventListener),
    History(HistoryListener),
    Render(AnimationFrame)
}
pub enum Event {
    Event(Event2),
    CustomEvent(CustomEvent)
}
pub struct EventStream {
    sender: Rc<RefCell<mpsc::UnboundedSender<Event>>>,
    receiver: mpsc::UnboundedReceiver<Event>,
    _listener: Listener,
}
impl EventStream {
    fn new<O>(target: &EventTarget, event_type: impl Into<Cow<'static, str>>, options: O) -> Self
    where O: Into<Option<EventListenerOptions>> {
        let options: Option<EventListenerOptions> = options.into();
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            EventListener::new_with_options(&target, event_type, options.unwrap_or_default(), move |event| {
                sender.borrow().unbounded_send(Event::Event(event.clone())).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::Event(listener),
        }
    }
    fn with_history<T>(history: &T, event_type: String) -> Self
    where T: History {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            history.listen(move || {
                sender.borrow().unbounded_send(Event::CustomEvent(CustomEvent::new(&event_type[..]).unwrap_throw())).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::History(listener),
        }
    }
    fn with_request_animation_frame(event_type: String) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            render::request_animation_frame(move |timestamp| {
                #[derive(Deserialize, Serialize)]
                struct Detail {
                    timestamp: f64
                }
                let detail = Detail {timestamp};
                let detail = serde_wasm_bindgen::to_value(&detail).unwrap_throw();
                let custom_event = CustomEvent::new(&event_type[..]).unwrap_throw();
                custom_event.init_custom_event_with_can_bubble_and_cancelable_and_detail(
                    &event_type[..],
                    false,
                    true,
                    &detail
                );
                sender.borrow().unbounded_send(Event::CustomEvent(custom_event)).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::Render(listener),
        }
    }
    #[allow(unused)]
    #[must_use]
    pub fn on<CB, Fut, O>(target: &EventTarget, event_type: impl Into<Cow<'static, str>>, options: O, callback: CB) -> impl FnOnce()
    where O: Into<Option<Options>>,
          CB: Fn(Event2) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let options = Into::<Option<Options>>::into(options).unwrap_or_default();
        let stream = Self::new(target, event_type, options.event_listener_options());
        let sender = Rc::clone(&stream.sender);
        let callback = move |event| {
            if let Event::Event(event) =  event {
                callback(event).map(|result| result.unwrap_throw())
            } else {
                wasm_bindgen::throw_str("DOM 事件仅支持 Event 事件类型")
            }
        };
        wasm_bindgen_futures::spawn_local(if options.is_serial() {
            stream.for_each(callback).left_future()
        } else {
            stream.for_each_concurrent(None, callback).right_future()
        });
        move || {
            let mut sender = sender.borrow_mut();
            executor::block_on(sender.close()).unwrap_throw()
        }
    }
    #[allow(unused)]
    #[must_use]
    pub fn on_history<CB, Fut, O, T>(history: &T, event_type: &str, is_serial: bool, callback: CB) -> impl FnOnce()
    where T: History,
          CB: Fn(CustomEvent) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_history(history, event_type.to_string());
        let sender = Rc::clone(&stream.sender);
        let callback = move |event| {
            if let Event::CustomEvent(event) =  event {
                callback(event).map(|result| result.unwrap_throw())
            } else {
                wasm_bindgen::throw_str("历史栈变更事件仅支持 CustomEvent 事件类型")
            }
        };
        wasm_bindgen_futures::spawn_local(if is_serial {
            stream.for_each(callback).left_future()
        } else {
            stream.for_each_concurrent(None, callback).right_future()
        });
        move || {
            let mut sender = sender.borrow_mut();
            executor::block_on(sender.close()).unwrap_throw()
        }
    }
    #[allow(unused)]
    #[must_use]
    pub fn on_request_animation_frame<CB, Fut>(event_type: &str, is_serial: bool, callback: CB) -> impl FnOnce()
    where CB: Fn(CustomEvent) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_request_animation_frame(event_type.to_string());
        let sender = Rc::clone(&stream.sender);
        let callback = move |event| {
            if let Event::CustomEvent(event) =  event {
                callback(event).map(|result| result.unwrap_throw())
            } else {
                wasm_bindgen::throw_str("帧渲染事件仅支持 CustomEvent 事件类型")
            }
        };
        wasm_bindgen_futures::spawn_local(if is_serial {
            stream.for_each(callback).left_future()
        } else {
            stream.for_each_concurrent(None, callback).right_future()
        });
        move || {
            let mut sender = sender.borrow_mut();
            executor::block_on(sender.close()).unwrap_throw()
        }
    }
}
impl Stream for EventStream {
    type Item = Event;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}