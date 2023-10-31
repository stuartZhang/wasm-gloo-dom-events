use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, StreamExt};
use ::gloo::render;
use ::serde::{Deserialize, Serialize};
use ::std::{cell::RefCell, future::Future, rc::Rc};
use ::wasm_bindgen::prelude::*;
use ::web_sys::{CustomEvent, CustomEventInit};
use super::{Event, EventStream, Listener};

impl EventStream {
    fn with_request_animation_frame() -> Self {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            render::request_animation_frame(move |timestamp| {
                sender.borrow().unbounded_send(Event::F64(timestamp)).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::Render(listener),
        }
    }
    /// 向浏览器【帧渲染】挂载事件处理函数
    /// * `event_type: &str`会被映射给事件处理函数 event 实参的 type 属性值
    /// * `is_serial: bool`表示：当事件被频繁且连续地被触发时，事件处理函数是否被允许并发地执行，或者必须串行执行
    /// * 事件处理函数实参`event: CustomEvent`的`detail.timestamp`属性值是`js - requestAnimationFrame(timestamp => {...})`中的`timestamp`回调函数实参值。
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
    #[must_use]
    pub fn on_request_animation_frame<CB, Fut>(event_type: String, is_serial: bool, mut callback: CB) -> impl FnOnce()
    where CB: FnMut(CustomEvent) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_request_animation_frame();
        let sender = Rc::clone(&stream.sender);
        let callback = move |event| {
            if let Event::F64(timestamp) =  event {
                #[derive(Deserialize, Serialize)]
                struct Detail {
                    timestamp: f64
                }
                let detail = Detail {timestamp};
                let detail = serde_wasm_bindgen::to_value(&detail).unwrap_throw();
                callback(CustomEvent::new_with_event_init_dict(
                    &event_type[..],
                    CustomEventInit::new().bubbles(false).cancelable(true).detail(&detail)
                ).unwrap_throw()).map(|result| result.unwrap_throw())
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