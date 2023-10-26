mod option;
use ::futures::{channel::mpsc, executor, FutureExt, SinkExt, stream::Stream, StreamExt};
use ::gloo::{events::{EventListener, EventListenerOptions}, history::{History, HistoryListener}, render::{self, AnimationFrame}, timers::callback::{Interval, Timeout}};
use ::serde::{Deserialize, Serialize};
use ::std::{borrow::Cow, cell::RefCell, convert::Into, future::Future, pin::Pin, rc::Rc, task::{Context, Poll}};
use ::wasm_bindgen::prelude::*;
use ::web_sys::{Event as Event2, CustomEvent, EventTarget};
pub use option::Options;
enum Listener {
    Event(EventListener),
    History(HistoryListener),
    Render(AnimationFrame),
    Interval(Interval),
    Timeout(Timeout)
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
    fn with_interval(event_type: String, duration: u32) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        let sender = Rc::new(RefCell::new(sender));
        let listener = {
            let sender = Rc::clone(&sender);
            Interval::new(duration, move || {
                let custom_event = CustomEvent::new(&event_type[..]).unwrap_throw();
                sender.borrow().unbounded_send(Event::CustomEvent(custom_event)).unwrap_throw();
            })
        };
        Self {
            sender,
            receiver,
            _listener: Listener::Interval(listener),
        }
    }
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
    /// 向`DOM`元素挂载指定事件类型的事件处理函数
    /// * `options: Into<Option<Options>>`被用来构造`gloo::events::EventListenerOptions`实例。
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
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
    /// 向浏览器【历史栈】挂载活跃项变更事件处理函数
    /// * `event_type: &str`会被映射给事件处理函数 event 实参的 type 属性值
    /// * `is_serial: bool`表示：当事件被频繁且连续地被触发时，事件处理函数是否被允许并发地执行，或者必须串行执行。
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
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
    /// 向浏览器【帧渲染】挂载事件处理函数
    /// * `event_type: &str`会被映射给事件处理函数 event 实参的 type 属性值
    /// * `is_serial: bool`表示：当事件被频繁且连续地被触发时，事件处理函数是否被允许并发地执行，或者必须串行执行
    /// * 事件处理函数实参`event: CustomEvent`的`detail.timestamp`属性值是`js - requestAnimationFrame(timestamp => {...})`中的`timestamp`回调函数实参值。
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
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
    /// 向浏览器【循环计划任务】挂载事件处理函数
    /// * `event_type: &str`会被映射给事件处理函数 event 实参的 type 属性值
    /// * `duration: u32` 事件的触发间隔周期
    /// * `is_serial: bool`表示：当事件被频繁且连续地被触发时，事件处理函数是否被允许并发地执行，或者必须串行执行
    /// # Examples
    /// # Panics
    /// # Errors
    /// # Safety
    #[must_use]
    pub fn on_interval<CB, Fut>(event_type: &str, duration: u32, is_serial: bool, callback: CB) -> impl FnOnce()
    where CB: Fn(CustomEvent) -> Fut + 'static,
          Fut: Future<Output = Result<(), JsValue>> + 'static {
        let stream = Self::with_interval(event_type.to_string(), duration);
        let sender = Rc::clone(&stream.sender);
        let callback = move |event| {
            if let Event::CustomEvent(event) =  event {
                callback(event).map(|result| result.unwrap_throw())
            } else {
                wasm_bindgen::throw_str("循环计划任务事件仅支持 CustomEvent 事件类型")
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
impl Stream for EventStream {
    type Item = Event;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}