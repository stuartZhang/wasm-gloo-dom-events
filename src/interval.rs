use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, StreamExt};
use ::gloo::timers::callback::Interval;
use ::std::{cell::RefCell, future::Future, rc::Rc};
use ::wasm_bindgen::prelude::*;
use ::web_sys::CustomEvent;
use super::{Event, EventStream, Listener, Vm};

impl EventStream {
    fn with_interval(duration: u32) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            Interval::new(duration, move || {
                sender.borrow().unbounded_send(Event::None).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::Interval(listener),
        }
    }
    /// 向浏览器【循环计划任务】挂载事件处理函数
    /// * `event_type: &str`会被映射给事件处理函数 event 实参的 type 属性值
    /// * `duration: u32` 事件的触发间隔周期
    /// * `is_serial: bool`表示：当事件被频繁且连续地被触发时，事件处理函数是否被允许并发地执行，或者必须串行执行
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
    #[must_use]
    pub fn on_interval<CB, Fut>(event_type: String, duration: u32, is_serial: bool, mut callback: CB) -> impl FnOnce()
    where CB: FnMut(Vm) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_interval(duration);
        let sender = Rc::clone(&stream.sender);
        let callback = move |_| callback(CustomEvent::new(&event_type).map_or_else(|_| {
            Vm::Nodejs(event_type.clone())
        }, |event| {
            Vm::Browser(event)
        })).map(|result| result.unwrap_throw());
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