use plutonium_engine::{
    app::run_app,
    pluto_objects::text2d::{HorizontalAlignment, TextContainer, VerticalAlignment},
    text::{DEFAULT_MSDF_MIN_PX, DEFAULT_TINY_RASTER_MAX_PX},
    utils::{Position, Rectangle},
    PlutoniumEngine, WindowConfig,
};

const SAMPLE_SIZES: [f32; 11] = [
    10.0, 12.0, 14.0, 16.0, 20.0, 24.0, 32.0, 40.0, 56.0, 72.0, 96.0,
];
const SAMPLE_TEXT: &str = "Sphinx of black quartz, judge my vow 0123456789 !?@#$%";
const RASTER_COMPARE_SIZE: f32 = 32.0;
const DEBUG_PAIR_TEXT: &str = "ju";
const DEBUG_PAIR_TEXT_RT: &str = "rt";
const DEBUG_PAIR_SMALL_SIZE: f32 = 12.0;
const DEBUG_PAIR_LARGE_SIZE: f32 = 24.0;

fn queue_line(
    engine: &mut PlutoniumEngine,
    text: &str,
    pos: Position,
    font_key: &str,
    size: f32,
    color: [f32; 4],
    z: i32,
) {
    let container = TextContainer::new(Rectangle::new(pos.x, pos.y, 900.0, size * 1.4))
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
        title: "MSDF Text Sanity Test".to_string(),
        width: 980,
        height: 1060,
    };

    let mut initialized = false;
    let mut printed_debug_once = false;
    let font_path = format!("{}/examples/media/roboto.ttf", env!("CARGO_MANIFEST_DIR"));
    let atlas_path = format!(
        "{}/examples/media/roboto.msdf.png",
        env!("CARGO_MANIFEST_DIR")
    );
    let metadata_path = format!(
        "{}/examples/media/roboto.msdf.json",
        env!("CARGO_MANIFEST_DIR")
    );

    run_app(config, move |engine, frame, _app| {
        if !initialized {
            if let Err(err) = engine.load_msdf_font_with_tiny_raster(
                &font_path,
                &atlas_path,
                &metadata_path,
                "roboto_msdf",
            ) {
                panic!(
                    "failed to load baked MSDF font: {:?}\n\n\
                     Bake once, then re-run this example:\n\
                       cargo run --bin msdf_bake -- --font \"{}\" --out-dir \"{}/examples/media\" --name roboto\n\n\
                     Expected files:\n\
                       {}\n\
                       {}",
                    err,
                    font_path,
                    env!("CARGO_MANIFEST_DIR"),
                    atlas_path,
                    metadata_path
                );
            }
            if let Err(err) = engine.load_font(&font_path, RASTER_COMPARE_SIZE, "roboto_raster") {
                panic!("failed to load raster font for comparison: {:?}", err);
            }
            initialized = true;
        }

        if frame.mouse_info.is_lmb_clicked {
            let msdf_out = "debug_msdf_font_atlas.png";
            match engine.debug_dump_font_atlas_png("roboto_msdf", msdf_out) {
                Ok(()) => println!("wrote {}", msdf_out),
                Err(err) => eprintln!("failed to write {}: {}", msdf_out, err),
            }
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
            "MSDF Only (load_msdf_font)",
            Position { x: 42.0, y: 42.0 },
            "roboto_msdf",
            30.0,
            [0.90, 0.95, 1.0, 1.0],
            40,
        );

        let mut cursor_y = 94.0;
        let mut compare_debug_y: Option<f32> = None;
        let mut compare_debug_text: Option<String> = None;
        for size in SAMPLE_SIZES {
            queue_line(
                engine,
                &format!("{:>3.0}px  {}", size, SAMPLE_TEXT),
                Position {
                    x: 42.0,
                    y: cursor_y,
                },
                "roboto_msdf",
                size,
                [0.94, 0.98, 1.0, 1.0],
                40,
            );

            if (size - RASTER_COMPARE_SIZE).abs() < f32::EPSILON {
                let compare_text = format!("{:>3.0}px  {}", size, SAMPLE_TEXT);
                queue_line(
                    engine,
                    &compare_text,
                    Position {
                        x: 42.0,
                        y: cursor_y + size * 1.40,
                    },
                    "roboto_raster",
                    size,
                    [1.0, 0.90, 0.72, 1.0],
                    40,
                );
                compare_debug_y = Some(cursor_y + size * 2.15);
                compare_debug_text = Some(compare_text);
                cursor_y += size * 2.80 + 10.0;
            } else {
                cursor_y += size * 1.38 + 10.0;
            }
        }

        if let (Some(debug_y), Some(compare_text)) = (compare_debug_y, compare_debug_text) {
            let (msdf_w, msdf_lines) = engine.measure_text(
                &compare_text,
                "roboto_msdf",
                0.0,
                0.0,
                Some(RASTER_COMPARE_SIZE),
            );
            let (raster_w, raster_lines) = engine.measure_text(
                &compare_text,
                "roboto_raster",
                0.0,
                0.0,
                Some(RASTER_COMPARE_SIZE),
            );
            let delta = msdf_w - raster_w;
            let pct = if raster_w.abs() > f32::EPSILON {
                (delta / raster_w) * 100.0
            } else {
                0.0
            };

            let tiny_cutoff = (DEFAULT_TINY_RASTER_MAX_PX + DEFAULT_MSDF_MIN_PX) * 0.5;
            let small_mode = if DEBUG_PAIR_SMALL_SIZE <= tiny_cutoff {
                "tiny+raster"
            } else {
                "msdf"
            };
            let large_mode = if DEBUG_PAIR_LARGE_SIZE <= tiny_cutoff {
                "tiny+raster"
            } else {
                "msdf"
            };
            let (ju_msdf_small, _) = engine.measure_text(
                DEBUG_PAIR_TEXT,
                "roboto_msdf",
                0.0,
                0.0,
                Some(DEBUG_PAIR_SMALL_SIZE),
            );
            let (ju_raster_small, _) = engine.measure_text(
                DEBUG_PAIR_TEXT,
                "roboto_raster",
                0.0,
                0.0,
                Some(DEBUG_PAIR_SMALL_SIZE),
            );
            let (ju_msdf_large, _) = engine.measure_text(
                DEBUG_PAIR_TEXT,
                "roboto_msdf",
                0.0,
                0.0,
                Some(DEBUG_PAIR_LARGE_SIZE),
            );
            let (ju_raster_large, _) = engine.measure_text(
                DEBUG_PAIR_TEXT,
                "roboto_raster",
                0.0,
                0.0,
                Some(DEBUG_PAIR_LARGE_SIZE),
            );
            let (rt_msdf_large, _) = engine.measure_text(
                DEBUG_PAIR_TEXT_RT,
                "roboto_msdf",
                0.0,
                0.0,
                Some(DEBUG_PAIR_LARGE_SIZE),
            );
            let (rt_raster_large, _) = engine.measure_text(
                DEBUG_PAIR_TEXT_RT,
                "roboto_raster",
                0.0,
                0.0,
                Some(DEBUG_PAIR_LARGE_SIZE),
            );
            let _ = debug_y;
            if !printed_debug_once {
                println!(
                    "[MSDF DEBUG] @32px widths: msdf={:.2}px ({} line) | raster={:.2}px ({} line) | delta={:+.2}px ({:+.2}%)",
                    msdf_w, msdf_lines, raster_w, raster_lines, delta, pct
                );
                println!(
                    "[MSDF DEBUG] pair \"{}\": {:.0}px [{}] msdf={:.2}px raster={:.2}px | {:.0}px [{}] msdf={:.2}px raster={:.2}px | tiny cutoff <= {:.1}px",
                    DEBUG_PAIR_TEXT,
                    DEBUG_PAIR_SMALL_SIZE,
                    small_mode,
                    ju_msdf_small,
                    ju_raster_small,
                    DEBUG_PAIR_LARGE_SIZE,
                    large_mode,
                    ju_msdf_large,
                    ju_raster_large,
                    tiny_cutoff
                );
                println!(
                    "[MSDF DEBUG] pair \"{}\": {:.0}px [{}] msdf={:.2}px raster={:.2}px",
                    DEBUG_PAIR_TEXT_RT,
                    DEBUG_PAIR_LARGE_SIZE,
                    large_mode,
                    rt_msdf_large,
                    rt_raster_large
                );
                if let Err(err) = engine.debug_print_text_line_layout(
                    DEBUG_PAIR_TEXT,
                    "roboto_msdf",
                    DEBUG_PAIR_SMALL_SIZE,
                    0.0,
                    0.0,
                ) {
                    eprintln!("failed debug_print_text_line_layout (msdf small): {}", err);
                }
                if let Err(err) = engine.debug_print_text_line_layout(
                    DEBUG_PAIR_TEXT,
                    "roboto_raster",
                    DEBUG_PAIR_SMALL_SIZE,
                    0.0,
                    0.0,
                ) {
                    eprintln!(
                        "failed debug_print_text_line_layout (raster small): {}",
                        err
                    );
                }
                if let Err(err) = engine.debug_print_text_line_layout(
                    DEBUG_PAIR_TEXT,
                    "roboto_msdf",
                    DEBUG_PAIR_LARGE_SIZE,
                    0.0,
                    0.0,
                ) {
                    eprintln!("failed debug_print_text_line_layout (msdf large): {}", err);
                }
                if let Err(err) = engine.debug_print_text_line_layout(
                    DEBUG_PAIR_TEXT,
                    "roboto_raster",
                    DEBUG_PAIR_LARGE_SIZE,
                    0.0,
                    0.0,
                ) {
                    eprintln!(
                        "failed debug_print_text_line_layout (raster large): {}",
                        err
                    );
                }
                if let Err(err) = engine.debug_print_text_line_layout(
                    DEBUG_PAIR_TEXT_RT,
                    "roboto_msdf",
                    DEBUG_PAIR_LARGE_SIZE,
                    0.0,
                    0.0,
                ) {
                    eprintln!("failed debug_print_text_line_layout (msdf rt): {}", err);
                }
                if let Err(err) = engine.debug_print_text_line_layout(
                    DEBUG_PAIR_TEXT_RT,
                    "roboto_raster",
                    DEBUG_PAIR_LARGE_SIZE,
                    0.0,
                    0.0,
                ) {
                    eprintln!("failed debug_print_text_line_layout (raster rt): {}", err);
                }
                printed_debug_once = true;
            }
        }

        queue_line(
            engine,
            "Left-click: dump debug_msdf_font_atlas.png",
            Position {
                x: 34.0,
                y: h - 40.0,
            },
            "roboto_msdf",
            18.0,
            [0.78, 0.84, 0.92, 1.0],
            40,
        );

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
