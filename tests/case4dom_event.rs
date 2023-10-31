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
    let document = utils::document();
    let body = utils::body().dyn_into::<HtmlBodyElement>().unwrap_throw();
    let button = create_element::<HtmlButtonElement>(&document, "button");
    body.append_child(&button).unwrap_throw();
    let deferred_future = LocalDeferredFuture::default();
    let defer = deferred_future.defer();
    let off = EventStream::on(&button, "click", Options::enable_prevent_default(true), move |_event| {
        defer.borrow_mut().complete("12".to_string());
        future::ready(Ok(()))
    });
    wasm_bindgen_futures::spawn_local(async move {
        TimeoutFuture::new(500).await;
        let event = PointerEvent::new("click").unwrap_throw();
        button.dispatch_event(&event).unwrap_throw();
    });
    let result = deferred_future.await;
    assert_eq!(result, "12");
    off();
}
fn create_element<T: JsCast>(document: &Document, tag_name: &str) -> T {
    document.create_element(tag_name).unwrap_throw().dyn_into::<T>().unwrap_throw()
}