#![forbid(unsafe_code)]

use eframe::WebRunner;
use executable_visualizer_lib::app::ExampleApp;
use executable_visualizer_lib::sections::ExecutableFile;
use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlElement, Request, RequestInit, RequestMode, Response};

pub fn main() {
    wasm_bindgen_futures::spawn_local(run());
}

async fn run() {
    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    let document = web_sys::window().unwrap().document().unwrap();

    let body = document.body().unwrap();

    // TODO: Maybe we could upstream this boilerplate into eframe
    let canvas = document.create_element("canvas").unwrap();
    canvas.set_id("the-id");
    body.append_child(&canvas)
        .expect("Append canvas to HTML body");
    body.style()
        .set_css_text("margin: 0; height: 100%; width: 100%");
    document
        .document_element()
        .unwrap()
        .dyn_ref::<HtmlElement>()
        .unwrap()
        .style()
        .set_css_text("margin: 0; height: 100%; width: 100%");

    let mut files = vec![ExecutableFile::load_dummy()];
    let data = load_example_binary("x86-executable-visualizer").await;
    if let Ok(file) = ExecutableFile::load_from_bytes("x86-executable-visualizer".to_owned(), &data)
    {
        files.push(file);
    }

    let app = ExampleApp::new(files);
    let runner = WebRunner::new();
    runner
        .start(
            "the-id",
            eframe::WebOptions::default(),
            Box::new(|_| Box::new(app)),
        )
        .await
        .unwrap();
}

async fn load_example_binary(name: &str) -> Vec<u8> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(name, &opts).unwrap();

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .unwrap();

    // `resp_value` is a `Response` object.
    assert!(resp_value.is_instance_of::<Response>());
    let resp: Response = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let js_value = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap();
    Uint8Array::new(&js_value).to_vec()
}
