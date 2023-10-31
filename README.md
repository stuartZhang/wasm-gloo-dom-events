# wasm-gloo-dom-events

二次封装[gloo crate](https://docs.rs/gloo/latest/gloo/index.html)，将`Cpp - RAII`风格的`DOM`事件处理函数**挂载方式**封装变形为`Typescript - Angular`风格的`register / deregister`模式。

## 创作动机

就`DOM`事件处理函数的【挂/卸载】操作而言，`gloo crate`已经做了非常完善的`RAII with Guard`设计模式封装。这包括：

1. 将【调用端】提供的`Rust`事件处理闭包封装成`wasm_bindgen::closure::Closure`。再，
2. 将`wasm_bindgen::closure::Closure`类型转换为`js_sys::Function`。接着，
3. 将`js_sys::Function`注入`DOM`元素`web_sys::EventTarget::add_event_listener_with_callback(&self, ...)` — 至此，完成了`DOM`事件处理函数的挂载工作。然后，
4. 构造与返回一个“保活守卫 — `RAII Guard`实例”给【调用端】。这就
5. 将`DOM`事件处理函数的卸载工作委托给`rustc`的`Drop Checker`来完成。后续，
6. 只要（来自`#4`的）`RAII Guard`实例被释放，`RAII Guard`的析构函数`Drop::drop(self)`就会卸载在`#3`挂载的`DOM`事件处理函数。

很完美！它

* 既将`DOM`事件处理函数的挂载操作委托给`RAII Guard`的构造器[EventListener::new(...)](https://docs.rs/gloo/latest/gloo/events/struct.EventListener.html#method.new)；同时，
* 又将同一个`DOM`事件处理函数的卸载操作委托给`RAII Guard`的析构器。这实在太`Thinking in Rust`了。

而且，能完全落实这套`RAII`编程范式的`Cpp`程序员也必定是老司机了。但，

1. `RAII Guard`是纯【系统编程】概念
2. `RAII Guard`实例是`WebAssembly`**线性内存**对象，却不在**`JS`堆**上
3. `RAII Guard`实例与`JS`事件循环没有直接的“物理”联系

所以，`RAII Guard`实例不会因为事件挂载操作而常驻内存（— 这是拥有`GC`加持的`js`程序才具备的“超能力”）。请看下面`js`代码片段：

```javascript
(() => {
    let handle = event => console.log(event.type);
    button.addEventListener('click', handle);
})();
// 至此，虽然函数执行结束，但`handle`闭包还驻留在内存中 — 这是事件循环的作用。
// 所以，`button`的`click`事件依旧有响应
```

相反，`RAII Guard`实例会随着【调用函数】的执行结束而被立即析构掉。进而，`Rust`端的`DOM`事件处理闭包也会被级联地释放掉。请看下面`rust`代码片段：

```rust
fn main() {
    let handle = EventListener::new(&button, "click", move |event| {
        info!("按钮点击事件2", event);
    });
}
// 在`Trunk`的入口函数`main()`执行结束之后，`button`的`click`处理函数
// 就被立即卸载了。所以，从网页点击`button`组件将不会获得任何的响应。
```

这明确不是我们想要的。我们想要是

1. `RAII Guard`实例常驻内存，和让`Rust - WASM`端的【`DOM`事件处理闭包】长期有效。**但又**
2. 禁止“人为刻意地”内存泄漏。比如，对`RAII Guard`实例**危险地**调用`std::mem::forget()` — 纯理论备选方案。**同时，也**
3. 避免使用`static mut`变量加`unsafe`块，全局缓存`RAII Guard`实例 — 这个法子是真管用，但**太外行**。请看下面代码片段：

    ```rust
    static mut HANDLE_CACHE: Option<EventListener> = None;
    fn main() {
        let handle = EventListener::new(&button, "click", move |event| {
            info!("按钮点击事件2", event);
        });
        unsafe { // 我想吐槽：“能写出这样代码的‘货’也真没谁了！”。
            HANDLE_CACHE.replace(handle);
        }
    }
    ```

归纳起来，我们期望由`DOM`事件挂载函数`gloo::events::EventListener::new(...)`返回的不是“保活守卫`Liveness Guard`”，而是“卸载函数`Deregistration Function`”。这样才和主流`UI`开发框架共同维系的编程习惯一致。目前，`register / deregister`事件挂载模式的经典用例就是`Angular`框架中的`$watch`监听器。比如，

```javascript
let offHandle;
vm.$onInit = () => {
    // 监听器挂载函数返回的是“卸载函数”。
    offHandle = $rootScope.$watch('some_property', () => {/* do something */});
};
vm.$onDestroy = () => {
    offHandle(); // 执行“卸载函数”注销掉监听器。
};
```

## 工作原理

1. 将`DOM`监听器作为“消息源”
2. 借助“异步、无限（缓存）、多进单出”信道[futures::channel::mpsc::unbounded](https://docs.rs/futures/0.3.28/futures/channel/mpsc/fn.unbounded.html)，将被触发的`DOM`**事件序列**转换成【异步流[futures::stream::Stream<Item = web_sys::Event>](https://docs.rs/futures/0.3.28/futures/stream/trait.Stream.html)】。
   1. 异步流的迭代项就是`DOM`事件对象`web_sys::Event`自身。
3. 借助`wasm_bindgen_futures::spawn_local()`执行器，将【异步流】实例挂到`js vm`的事件循环上。进而，确保【异步流】实例在`WebAssembly`**线性内存**中的常驻，除非我们显式地卸载它。
4. 于是，【调用端】只要`futures::stream::StreamExt::for_each`（甚至，**并发**`for_each`）该【异步流】实例，就能在
   1. 在`Trunk`的入口函数`main`执行结束之后，
   2. 依旧持续监听与处理由`DOM`元素发起的事件了。

【异步编程】真是前端的技术关键路线，无论是`Typescript`前端，还是`WEB`汇编前端。

## 功能描述

首先，该`crate`分别对

1. `DOM`元素触发事件
2. 浏览器【历史栈】变更事件`window.addEventListener('popstate',...)`
3. 浏览器【帧渲染】事件`requestAnimationFrame()`
4. `setTimeout()`
5. `setInterval()`

的处理函数【挂/卸载】操作做了`register / deregister`封装。

其次，对非常活跃事件源的事件处理函数，基于【异步流】底层技术，提供两种执行方式：

1. 绝对地**串行**执行。无论事件处理函数是**同步**函数，还是**异步**函数，程序都会确保前一个事件处理函数被完全执行结果之后，才会开始执行后一个事件处理函数。
2. **并发**执行（注：不是**并行**执行，因为未涉及多线程，而是多协程）。一旦前一个事件处理函数进入了`.await`状态，剩余事件处理函数就立即开始执行或继续执行。

至于，如何传参配置执行方式，请见程序的【文档注释】。

## 安装

```shell
cargo add wasm-gloo-dom-events
```

## 调用套路详解

一共分成五个场景与五类套路

### 浏览器`DOM`元素响应事件

```rust
use ::deferred_future::LocalDeferredFuture;
use ::futures::future;
use ::gloo::{timers::future::TimeoutFuture, utils};
use ::wasm_bindgen::{JsCast, UnwrapThrowExt};
use ::wasm_bindgen_test::*;
use ::wasm_gloo_dom_events::{EventStream, Options};
use ::web_sys::{Document, HtmlBodyElement, HtmlButtonElement, PointerEvent};
wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn dom_event() {
    //
    // 创建一个按钮`DOM`元素，和将其添加至文档`DOM`流中。
    //
    let document = utils::document();
    let body = utils::body().dyn_into::<HtmlBodyElement>().unwrap_throw();
    let button = create_element::<HtmlButtonElement>(&document, "button");
    body.append_child(&button).unwrap_throw();
    let deferred_future = LocalDeferredFuture::default();
    let defer = deferred_future.defer();
    //
    // 给按钮`DOM`元素挂载鼠标点击事件处理函数。
    // 1. 回调函数唯一形参是`DOM`事件自身的事件对象。
    //
    let off = EventStream::on(&button, "click", Options::enable_prevent_default(true), move |_event| {
        // 异步的事件处理函数
        defer.borrow_mut().complete("12".to_string());
        future::ready(Ok(()))
    });
    //
    // 模拟稍后点击按钮`DOM`元素。
    //
    wasm_bindgen_futures::spawn_local(async move {
        TimeoutFuture::new(500).await;
        let event = PointerEvent::new("click").unwrap_throw();
        button.dispatch_event(&event).unwrap_throw();
    });
    let result = deferred_future.await;
    assert_eq!(result, "12");
    //
    // 卸载事件处理函数
    //
    off();
}
fn create_element<T: JsCast>(document: &Document, tag_name: &str) -> T {
    document.create_element(tag_name).unwrap_throw().dyn_into::<T>().unwrap_throw()
}
```

从命令行，执行命令`wasm-pack test --chrome --headless --test=case4dom_event`可直接运行此例程。

### 浏览器【历史栈】变更事件

```rust
use ::deferred_future::LocalDeferredFuture;
use ::futures::future;
use gloo::history::History;
use ::gloo::{history::BrowserHistory, timers::future::TimeoutFuture};
use ::std::rc::Rc;
use ::wasm_bindgen_test::*;
use ::wasm_gloo_dom_events::EventStream;
wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn history() {
    //
    // 从主窗体拾取出`history`实例
    //
    let browser_history = Rc::new(BrowserHistory::new());
    let deferred_future: LocalDeferredFuture<String> = LocalDeferredFuture::default();
    let defer = deferred_future.defer();
    let off = {
        let browser_history = Rc::clone(&browser_history);
        //
        // 给`history`挂载历史栈更新事件处理函数。
        // 1. 回调函数第一个形参是`CustomEvent`。其`type`属性值呼应于`EventStream::on_history(..)`的第二个实参值。
        // 2. 回调函数第二个形参是`history`的最新状态数据。
        //
        EventStream::on_history(Rc::clone(&browser_history), "测试".to_string(), true, move |_event, state: Option<Rc<&str>>| {
            // 异步的事件处理函数
            defer.borrow_mut().complete(state.unwrap().to_string());
            future::ready(Ok(()))
        })
    };
    {
        let browser_history = Rc::clone(&browser_history);
        //
        // 模拟稍后`TAB`签路由变更 — 浏览器地址栏内容发生变化。
        //
        wasm_bindgen_futures::spawn_local(async move {
            TimeoutFuture::new(500).await;
            // 修改地址栏`url`，和压栈新历史状态数据。在本例中，
            // 1. 修改浏览器地址栏为`/route1`
            // 2. 填入历史状态数据"12"字符串
            browser_history.push_with_state("route1", "12");
        });
    }
    let result = deferred_future.await;
    assert_eq!(result, "12");
    //
    // 卸载事件处理函数
    //
    off();
}
```

从命令行，执行命令`wasm-pack test --chrome --headless --test=case4history`可直接运行此例程。

### 浏览器【帧渲染】事件

```rust
use ::deferred_future::LocalDeferredFuture;
use ::futures::future;
use ::wasm_bindgen_test::*;
use ::wasm_gloo_dom_events::EventStream;
wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn request_animation_frame() {
    let deferred_future = LocalDeferredFuture::default();
    let defer = deferred_future.defer();
    //
    // 给浏览器【帧渲染】挂载事件。回调函数唯一形参是`CustomEvent`。
    // 1. 其`type`属性值呼应于`EventStream::on_request_animation_frame(..)`的第一个实参值。
    // 2. 其`detail.timestamp`属性值是`js - requestAnimationFrame(timestamp => {...})`中的`timestamp`回调函数实参值。
    //
    let off = EventStream::on_request_animation_frame("requestAnimationFrame".to_string(), true, move |_event| {
        // 异步的事件处理函数
        defer.borrow_mut().complete("12".to_string());
        future::ready(Ok(()))
    });
    let result = deferred_future.await;
    assert_eq!(result, "12");
    //
    // 卸载事件处理函数
    //
    off();
}
```

从命令行，执行命令`wasm-pack test --chrome --headless --test=case4request_animation_frame`可直接运行此例程。

### 单次计划任务

```rust
use ::deferred_future::LocalDeferredFuture;
use ::futures::future;
use ::wasm_bindgen_test::*;
use ::wasm_gloo_dom_events::EventStream;
#[cfg(not(feature = "nodejs"))]
wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn timeout() {
    let deferred_future = LocalDeferredFuture::default();
    let defer = deferred_future.defer();
    //
    // 给`window.setTimeout()`挂载回调函数。回调函数唯一形参是`CustomEvent`。
    // 1. 其`type`属性值呼应于`EventStream::on_timeout(..)`的第一个实参值。
    //
    let off = EventStream::on_timeout("timeout".to_string(), 1000, move |_event| {
        // 异步的事件处理函数
        defer.borrow_mut().complete("12".to_string());
        future::ready(Ok(()))
    });
    let result = deferred_future.await;
    assert_eq!(result, "12");
    //
    // 卸载事件处理函数
    //
    off();
}
```

从命令行，执行命令可直接运行此例程

* 浏览器：`wasm-pack test --chrome --headless --test=case4timeout`
* `nodejs`：`wasm-pack test --node --features=nodejs --test=case4timeout`

### 周期多次计划任务

```rust
use ::deferred_future::LocalDeferredFuture;
use ::futures::future;
use ::wasm_bindgen_test::*;
use ::wasm_gloo_dom_events::EventStream;
#[cfg(not(feature = "nodejs"))]
wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn timeout() {
    let deferred_future = LocalDeferredFuture::default();
    let defer = deferred_future.defer();
    let mut count = 0_u8;
    //
    // 给`window.setInterval()`挂载回调函数。回调函数唯一形参是`CustomEvent`。
    // 1. 其`type`属性值呼应于`EventStream::on_interval(..)`的第一个实参值。
    //
    let off = EventStream::on_interval("interval".to_string(), 1000, true, move |_event| {
        // 异步的事件处理函数
        count += 1;
        if count > 5 {
            defer.borrow_mut().complete("12".to_string());
        }
        future::ready(Ok(()))
    });
    let result = deferred_future.await;
    assert_eq!(result, "12");
    //
    // 卸载事件处理函数
    //
    off();
}
```

从命令行，执行命令可直接运行此例程

* 浏览器：`wasm-pack test --chrome --headless --test=case4interval`
* `nodejs`：`wasm-pack test --node --features=nodejs --test=case4interval`
