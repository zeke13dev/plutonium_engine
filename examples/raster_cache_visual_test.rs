use plutonium_engine::{
    app::run_app,
    pluto_objects::text2d::{HorizontalAlignment, TextContainer, VerticalAlignment},
    utils::{Position, Rectangle},
    FontLoadOptions, GlyphSet, PlutoniumEngine, PrewarmConfig, PrewarmPolicy, WindowConfig,
};

const SAMPLE_SIZES: [f32; 11] = [
    10.0, 12.0, 14.0, 16.0, 20.0, 24.0, 32.0, 40.0, 56.0, 72.0, 96.0,
];
const SAMPLE_TEXT: &str = "Sphinx of black quartz, judge my vow 0123456789 !?@#$%";
const FONT_KEY: &str = "roboto_raster_cache";
const BASE_LOAD_SIZE: f32 = 32.0;
const DEBUG_PAIR_A: &str = "ju";
const DEBUG_PAIR_B: &str = "rt";
const DEBUG_SMALL_SIZE: f32 = 12.0;
const DEBUG_LARGE_SIZE: f32 = 24.0;

fn raster_variant_key(base: &str, size: f32) -> String {
    format!(
        "{}::raster@{}",
        base,
        (size.max(1.0) * 100.0).round() as u32
    )
}

fn queue_line(
    engine: &mut PlutoniumEngine,
    text: &str,
    pos: Position,
    font_key: &str,
    size: f32,
    color: [f32; 4],
    z: i32,
) {
    let container = TextContainer::new(Rectangle::new(pos.x, pos.y, 940.0, size * 1.4))
        .with_alignment(HorizontalAlignment::Left, VerticalAlignment::Top)
        .with_padding(0.0);
    engine.queue_text_with_spacing(
        text,
        font_key,
        pos,
        &container,
        0.0,
        0.0,
        z,
        color,
        Some(size),
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WindowConfig {
        title: "Raster Cache Visual Test".to_string(),
        width: 1000,
        height: 1080,
    };

    let mut initialized = false;
    let mut printed_debug_once = false;
    let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));

    run_app(config, move |engine, frame, _app| {
        if !initialized {
            let options = FontLoadOptions {
                prewarm_policy: PrewarmPolicy::Custom(PrewarmConfig {
                    sizes: SAMPLE_SIZES.to_vec(),
                    glyph_set: GlyphSet::AsciiCore,
                }),
                ..FontLoadOptions::default()
            };
            if let Err(err) =
                engine.load_font_with_options(&font_path, BASE_LOAD_SIZE, FONT_KEY, options)
            {
                panic!("failed to load raster font cache test font: {:?}", err);
            }

            // Also warm explicitly so the example prints deterministic cache stats.
            let warm_stats = engine
                .warm_text_cache(
                    FONT_KEY,
                    PrewarmConfig {
                        sizes: SAMPLE_SIZES.to_vec(),
                        glyph_set: GlyphSet::AsciiCore,
                    },
                )
                .expect("warm_text_cache should succeed");
            println!(
                "[RASTER CACHE] requested={} warmed={} already_loaded={} glyphs_rasterized={}",
                warm_stats.requested_sizes,
                warm_stats.warmed_sizes,
                warm_stats.already_loaded_sizes,
                warm_stats.glyphs_rasterized
            );

            initialized = true;
        }

        if frame.mouse_info.is_lmb_clicked {
            let base_q = (BASE_LOAD_SIZE.max(1.0) * 100.0).round() as u32;
            for size in SAMPLE_SIZES {
                let size_q = (size.max(1.0) * 100.0).round() as u32;
                let key = if size_q == base_q {
                    FONT_KEY.to_string()
                } else {
                    raster_variant_key(FONT_KEY, size)
                };
                let out = format!("debug_raster_{}.png", size_q);
                if let Err(err) = engine.debug_dump_font_atlas_png(&key, &out) {
                    eprintln!("failed atlas dump for '{}' -> '{}': {}", key, out, err);
                }
            }
            println!("[RASTER CACHE] wrote debug_raster_*.png atlas dumps");
        }

        let w = engine.size.width as f32;
        let h = engine.size.height as f32;

        engine.begin_frame();
        engine.draw_rect(
            Rectangle::new(0.0, 0.0, w, h),
            [0.07, 0.08, 0.10, 1.0],
            0.0,
            None,
            0,
        );
        engine.draw_rect(
            Rectangle::new(20.0, 20.0, w - 40.0, h - 40.0),
            [0.11, 0.13, 0.17, 1.0],
            12.0,
            Some(([0.17, 0.21, 0.28, 1.0], 1.0)),
            1,
        );

        queue_line(
            engine,
            "Raster Cache (multi-size prewarm)",
            Position { x: 42.0, y: 42.0 },
            FONT_KEY,
            30.0,
            [0.90, 0.95, 1.0, 1.0],
            40,
        );

        let mut cursor_y = 94.0;
        for size in SAMPLE_SIZES {
            queue_line(
                engine,
                &format!("{:>3.0}px  {}", size, SAMPLE_TEXT),
                Position {
                    x: 42.0,
                    y: cursor_y,
                },
                FONT_KEY,
                size,
                [0.94, 0.98, 1.0, 1.0],
                40,
            );
            cursor_y += size * 1.38 + 10.0;
        }

        if !printed_debug_once {
            for size in SAMPLE_SIZES {
                let text = format!("{:>3.0}px  {}", size, SAMPLE_TEXT);
                let (w_px, lines) = engine.measure_text(&text, FONT_KEY, 0.0, 0.0, Some(size));
                println!(
                    "[RASTER CACHE] sample width @{:>4.0}px: {:.2}px ({} line)",
                    size, w_px, lines
                );
            }
            for &(pair, size) in &[
                (DEBUG_PAIR_A, DEBUG_SMALL_SIZE),
                (DEBUG_PAIR_A, DEBUG_LARGE_SIZE),
                (DEBUG_PAIR_B, DEBUG_LARGE_SIZE),
            ] {
                let (pair_w, _) = engine.measure_text(pair, FONT_KEY, 0.0, 0.0, Some(size));
                println!(
                    "[RASTER CACHE] pair {:?} @{:>4.0}px => {:.2}px",
                    pair, size, pair_w
                );
                if let Err(err) =
                    engine.debug_print_text_line_layout(pair, FONT_KEY, size, 0.0, 0.0)
                {
                    eprintln!(
                        "failed debug_print_text_line_layout ({:?} @{}px): {}",
                        pair, size, err
                    );
                }
            }
            printed_debug_once = true;
        }

        queue_line(
            engine,
            "Left-click: dump prewarmed raster atlases (debug_raster_*.png)",
            Position {
                x: 34.0,
                y: h - 40.0,
            },
            FONT_KEY,
            18.0,
            [0.78, 0.84, 0.92, 1.0],
            40,
        );

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
