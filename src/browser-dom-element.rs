mod option;

use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, StreamExt};
use ::gloo::events::{EventListener, EventListenerOptions};
use ::std::{borrow::Cow, cell::RefCell, convert::Into, future::Future, rc::Rc};
use ::wasm_bindgen::prelude::*;
use ::web_sys::{Event as Event2, EventTarget};
use super::{Event, EventStream, Listener};
pub use option::Options;

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
    /// 向`DOM`元素挂载指定事件类型的事件处理函数
    /// * `options: Into<Option<Options>>`被用来构造`gloo::events::EventListenerOptions`实例。
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
    #[must_use]
    pub fn on<CB, Fut, O>(target: &EventTarget, event_type: impl Into<Cow<'static, str>>, options: O, mut callback: CB) -> impl FnOnce()
    where O: Into<Option<Options>>,
          CB: FnMut(Event2) -> Fut + 'static,
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
}