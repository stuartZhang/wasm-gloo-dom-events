use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, StreamExt};
use ::gloo::timers::callback::Timeout;
use ::std::{cell::RefCell, future::Future, rc::Rc};
use ::wasm_bindgen::prelude::*;
use ::web_sys::CustomEvent;
use super::{Event, EventStream, Listener, Vm};

impl EventStream {
    fn with_timeout(duration: u32) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            Timeout::new(duration, move || {
                sender.borrow().unbounded_send(Event::None).unwrap_throw();
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
    pub fn on_timeout<CB, Fut>(event_type: String, duration: u32, mut callback: CB) -> impl FnOnce()
    where CB: FnMut(Vm) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_timeout(duration);
        let sender = Rc::clone(&stream.sender);
        let callback = move |_| callback(CustomEvent::new(&event_type).map_or_else(|_| {
            Vm::Nodejs(event_type.clone())
        }, |event| {
            Vm::Browser(event)
        })).map(|result| result.unwrap_throw());
        wasm_bindgen_futures::spawn_local(stream.for_each(callback));
        move || {
            let mut sender = sender.borrow_mut();
            executor::block_on(sender.close()).unwrap_throw()
        }
    }
}
