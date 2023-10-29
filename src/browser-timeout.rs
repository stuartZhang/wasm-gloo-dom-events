use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, StreamExt};
use ::gloo::timers::callback::Timeout;
use ::std::{cell::RefCell, future::Future, rc::Rc};
use ::wasm_bindgen::prelude::*;
use ::web_sys::CustomEvent;
use super::{Event, EventStream, Listener};

impl EventStream {
    fn with_timeout(event_type: String, duration: u32) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            Timeout::new(duration, move || {
                let custom_event = CustomEvent::new(&event_type[..]).unwrap_throw();
                sender.borrow().unbounded_send(Event::CustomEvent(custom_event)).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::Timeout(listener),
        }
    }
    /// 向浏览器【单次计划任务】挂载事件处理函数
    /// * `event_type: &str`会被映射给事件处理函数 event 实参的 type 属性值
    /// * `duration: u32` 事件的触发的延迟时间
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
    #[must_use]
    pub fn on_timeout<CB, Fut>(event_type: &str, duration: u32, callback: CB) -> impl FnOnce()
    where CB: Fn(CustomEvent) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_timeout(event_type.to_string(), duration);
        let sender = Rc::clone(&stream.sender);
        let callback = move |event| {
            if let Event::CustomEvent(event) =  event {
                callback(event).map(|result| result.unwrap_throw())
            } else {
                wasm_bindgen::throw_str("单次计划任务事件仅支持 CustomEvent 事件类型")
            }
        };
        wasm_bindgen_futures::spawn_local(stream.for_each(callback));
        move || {
            let mut sender = sender.borrow_mut();
            executor::block_on(sender.close()).unwrap_throw()
        }
    }
}
