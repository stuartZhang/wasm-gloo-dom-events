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
    let off = EventStream::on_timeout("timeout".to_string(), 1000, move |_event| {
        defer.borrow_mut().complete("12".to_string());
        future::ready(Ok(()))
    });
    let result = deferred_future.await;
    assert_eq!(result, "12");
    off();
}