#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();

    // SMITH_SPHERE_WINDOW=WIDTHxHEIGHT forces the initial size (re-applied on
    // the first frame by the app) and disables window-geometry persistence so
    // layout captures are reproducible.
    let size_override = std::env::var("SMITH_SPHERE_WINDOW").ok().and_then(|value| {
        let (w, h) = value.split_once('x')?;
        Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
    });

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        persist_window: size_override.is_none(),
        viewport: egui::ViewportBuilder::default()
            .with_title("SmithSphere 史密斯球")
            .with_inner_size(size_override.unwrap_or([1280.0, 820.0]))
            .with_min_inner_size([360.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "SmithSphere",
        native_options,
        Box::new(|context| Ok(Box::new(smith_sphere_gui::SmithSphereApp::new(context)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();

    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("browser window is unavailable");
        let document = window.document().expect("browser document is unavailable");
        let canvas = document
            .get_element_by_id("smith_sphere_canvas")
            .expect("smith_sphere_canvas element is missing")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("smith_sphere_canvas is not a canvas element");

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|context| Ok(Box::new(smith_sphere_gui::SmithSphereApp::new(context)))),
            )
            .await;

        if let Some(status) = document.get_element_by_id("startup_status") {
            match result {
                Ok(()) => status.remove(),
                Err(error) => {
                    status.set_text_content(Some("应用启动失败。"));
                    panic!("failed to start web application: {error:?}");
                }
            }
        }
    });
}
