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
    let browser_history = Rc::new(BrowserHistory::new());
    let deferred_future: LocalDeferredFuture<String> = LocalDeferredFuture::default();
    let defer = deferred_future.defer();
    let off = {
        let browser_history = Rc::clone(&browser_history);
        EventStream::on_history(Rc::clone(&browser_history), "测试".to_string(), true, move |_event, state: Option<Rc<&str>>| {
            defer.borrow_mut().complete(state.unwrap().to_string());
            future::ready(Ok(()))
        })
    };
    {
        let browser_history = Rc::clone(&browser_history);
        wasm_bindgen_futures::spawn_local(async move {
            TimeoutFuture::new(500).await;
            browser_history.push_with_state("route1", "12");
        });
    }
    let result = deferred_future.await;
    assert_eq!(result, "12");
    off();
}