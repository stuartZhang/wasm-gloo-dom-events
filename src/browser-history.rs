use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, StreamExt};
use ::gloo::history::History;
use ::std::{cell::RefCell, future::Future, rc::Rc};
use ::wasm_bindgen::prelude::*;
use ::web_sys::CustomEvent;
use super::{Event, EventStream, Listener};

impl EventStream {
    fn with_history<T>(history: Rc<T>) -> Self
    where T: History + 'static {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            Rc::clone(&history).listen(move || {
                sender.borrow().unbounded_send(Event::Location(history.location())).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::History(listener),
        }
    }
    /// 向浏览器【历史栈】挂载活跃项变更事件处理函数
    /// * `event_type: &str`会被映射给事件处理函数 event 实参的 type 属性值
    /// * `is_serial: bool`表示：当事件被频繁且连续地被触发时，事件处理函数是否被允许并发地执行，或者必须串行执行。
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
    #[must_use]
    pub fn on_history<S, T, CB, Fut>(history: Rc<T>, event_type: String, is_serial: bool, callback: CB) -> impl FnOnce()
    where S: 'static,
          T: History + 'static,
          CB: Fn(CustomEvent, Option<Rc<S>>) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_history(history);
        let sender = Rc::clone(&stream.sender);
        let callback = move |event| {
            if let Event::Location(location) =  event {
                let custom_event = CustomEvent::new(&event_type[..]).unwrap_throw();
                let state = location.state::<S>();
                callback(custom_event, state).map(|result| result.unwrap_throw())
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
}