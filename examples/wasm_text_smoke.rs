#[cfg(target_arch = "wasm32")]
use plutonium_engine::{
    app::{run_app_wasm_with_options, WasmAppConfig, WindowConfig},
    pluto_objects::text2d::TextContainer,
    utils::{Position, Rectangle},
};

#[cfg(target_arch = "wasm32")]
const MSDF_PNG: &[u8] = include_bytes!("media/roboto.msdf.png");
#[cfg(target_arch = "wasm32")]
const MSDF_JSON: &str = include_str!("media/roboto.msdf.json");
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
static SMOKE_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_arch = "wasm32")]
fn set_debug_status(message: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            document.set_title(message);
            if let Some(el) = document.get_element_by_id("pluto-debug") {
                el.set_text_content(Some(message));
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn run_smoke() {
    if SMOKE_STARTED.swap(true, Ordering::SeqCst) {
        set_debug_status("run_smoke already started");
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        set_debug_status("booting wasm app...");
        let config = WindowConfig {
            title: "Plutonium WASM Text Smoke".to_string(),
            width: 960,
            height: 540,
        };

        let mut loaded = false;
        let mut elapsed = 0.0f32;
        let mut frame_count: u64 = 0;
        let mut load_failed = false;
        let wasm_cfg = WasmAppConfig {
            // Debug-friendly: keep browser context menu and devtools interaction usable.
            prevent_default: false,
            focusable: true,
        };
        if let Err(err) = run_app_wasm_with_options(
            config,
            "pluto-canvas",
            wasm_cfg,
            move |engine, frame, _app| {
                frame_count = frame_count.wrapping_add(1);
                elapsed += frame.delta_time;
                if frame_count % 30 == 0 {
                    set_debug_status(&format!(
                        "alive frame={} dt={:.4} loaded={} failed={}",
                        frame_count, frame.delta_time, loaded, load_failed
                    ));
                }

                if !loaded && !load_failed {
                    // Use pre-baked MSDF font — no runtime rasterization, no main-thread stall.
                    if let Err(load_err) =
                        engine.load_msdf_font_from_png_bytes(MSDF_PNG, MSDF_JSON, "smoke")
                    {
                        web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format!(
                            "[wasm_text_smoke] failed to load msdf font: {:?}",
                            load_err
                        )));
                        set_debug_status("font load failed");
                        load_failed = true;
                        return;
                    }
                    loaded = true;
                    set_debug_status("font loaded (msdf)");
                }

                let pulse = 0.5 + 0.5 * (elapsed * 2.0).sin();
                let container = TextContainer::new(Rectangle::new(40.0, 40.0, 880.0, 230.0));

                engine.begin_frame();
                // Full-screen heartbeat so we can confirm frame submission even when text fails.
                engine.draw_rect(
                    Rectangle::new(0.0, 0.0, 960.0, 540.0),
                    [0.02 + pulse * 0.06, 0.05, 0.10, 1.0],
                    0.0,
                    None,
                    -5,
                );
                if load_failed {
                    engine.draw_rect(
                        Rectangle::new(0.0, 0.0, 960.0, 540.0),
                        [0.35, 0.05, 0.05, 1.0],
                        0.0,
                        None,
                        0,
                    );
                    engine.draw_rect(
                        Rectangle::new(40.0, 40.0, 880.0, 36.0),
                        [0.90, 0.20, 0.15, 1.0],
                        6.0,
                        None,
                        1,
                    );
                    let _ = engine.end_frame();
                    return;
                }
                engine.draw_rect(
                    Rectangle::new(24.0, 24.0, 912.0, 232.0),
                    [0.08, 0.10, 0.14, 1.0],
                    12.0,
                    None,
                    0,
                );
                engine.queue_text_with_spacing(
                    "WASM text smoke test",
                    "smoke",
                    Position { x: 56.0, y: 74.0 },
                    &container,
                    0.0,
                    0.0,
                    1,
                    [1.0, 1.0, 1.0, 1.0],
                    Some(54.0),
                );
                engine.queue_text_with_spacing(
                    "If you can read this, raster text rendering is alive.",
                    "smoke",
                    Position { x: 56.0, y: 140.0 },
                    &container,
                    0.0,
                    0.0,
                    1,
                    [0.84, 0.92, 0.96, 1.0],
                    Some(26.0),
                );
                engine.draw_rect(
                    Rectangle::new(56.0, 186.0, 400.0 * pulse.max(0.15), 8.0),
                    [0.20, 0.80, 0.55, 0.95],
                    4.0,
                    None,
                    1,
                );
                if let Err(end_err) = engine.end_frame() {
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format!(
                        "[wasm_text_smoke] frame end failed: {:?}",
                        end_err
                    )));
                    set_debug_status("end_frame failed");
                }
            },
        )
        .await
        {
            set_debug_status("bootstrap failed");
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format!(
                "[wasm_text_smoke] app bootstrap failed: {:?}",
                err
            )));
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("wasm_text_smoke is wasm-only. Use: scripts/run_wasm_text_smoke.sh");
}
