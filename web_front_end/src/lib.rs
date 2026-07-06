mod web_backend;

use crate::web_backend::WebBackend;
use ui_lib::lock_screen::LockScreen;
use wasm_bindgen::JsCast;

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub async fn start() -> eyre::Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();

    let canvas = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("main_canvas"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("Canvas not found"))?;

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|_| Ok(Box::new(LockScreen::new(WebBackend::new())))),
        )
        .await
}
