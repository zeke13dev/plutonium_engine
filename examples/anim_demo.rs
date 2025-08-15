#[cfg(not(feature = "anim"))]
fn main() {
    eprintln!("Run this example with --features anim");
}

#[cfg(feature = "anim")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use plutonium_engine::anim::{Ease, Timeline, Track, Tween};
    use plutonium_engine::app::run_app;
    use plutonium_engine::ui::{draw_focus_ring, FocusRingStyle, NinePatch};
    use plutonium_engine::utils::{Position, Rectangle, Size};
    use plutonium_engine::WindowConfig;
    use std::cell::Cell;
    use std::rc::Rc;

    let config = WindowConfig {
        title: "Anim Demo".into(),
        width: 800,
        height: 600,
    };

    // Position timeline: move A -> B, then back (sequence of two tweens)
    let mut pos_tl: Timeline<Position> = Timeline::new();
    pos_tl.push_track(Track::Sequence(vec![
        Tween::new(
            Position { x: 120.0, y: 120.0 },
            Position { x: 520.0, y: 360.0 },
            1.6,
            Ease::CubicBezier {
                x1: 0.42,
                y1: 0.0,
                x2: 0.58,
                y2: 1.0,
            },
        ),
        Tween::new(
            Position { x: 520.0, y: 360.0 },
            Position { x: 120.0, y: 120.0 },
            1.6,
            Ease::CubicBezier {
                x1: 0.42,
                y1: 0.0,
                x2: 0.58,
                y2: 1.0,
            },
        ),
    ]));

    // Width timeline: 160 -> 260 -> 160
    let mut width_tl: Timeline<f32> = Timeline::new();
    width_tl.push_track(Track::Sequence(vec![
        Tween::new(160.0f32, 260.0f32, 1.2, Ease::EaseInOut),
        Tween::new(260.0f32, 160.0f32, 1.2, Ease::EaseInOut),
    ]));

    // Add a label and callback: pulse the ring color when width hits the first keyframe end (t=1.2)
    width_tl.add_label("pulse", 1.2);
    let pulse_timer = Rc::new(Cell::new(0.0f32));
    {
        let pulse_for_cb = Rc::clone(&pulse_timer);
        width_tl.on_label("pulse", move || pulse_for_cb.set(0.4));
    }

    // Focus ring corner radius timeline: 8 -> 16 -> 8 (runs in parallel)
    let mut cr_tl: Timeline<f32> = Timeline::new();
    cr_tl.push_track(Track::Sequence(vec![
        Tween::new(8.0f32, 16.0f32, 0.9, Ease::EaseInOut),
        Tween::new(16.0f32, 8.0f32, 0.9, Ease::EaseInOut),
    ]));

    let mut current_pos = Position { x: 120.0, y: 120.0 };
    let mut current_w = 200.0f32;
    let mut current_cr = 12.0f32;

    let mut atlas_id: Option<uuid::Uuid> = None;
    let insets = [16.0, 16.0, 16.0, 16.0];
    let tile = Size {
        width: 64.0,
        height: 64.0,
    };

    run_app(config, move |engine, frame| {
        engine.begin_frame();

        if atlas_id.is_none() {
            let (atlas_key, _dims) = engine.create_texture_atlas(
                &format!(
                    "{}/examples/media/map_atlas.svg",
                    env!("CARGO_MANIFEST_DIR")
                ),
                Position { x: 0.0, y: 0.0 },
                tile,
            );
            atlas_id = Some(atlas_key);
        }

        // Advance timelines
        let pos_out = pos_tl.step(frame.delta_time);
        if let Some(track_vals) = pos_out.first() {
            if let Some(&v) = track_vals.last() {
                current_pos = v;
            }
        }
        let w_out = width_tl.step(frame.delta_time);
        if let Some(track_vals) = w_out.first() {
            if let Some(&v) = track_vals.last() {
                current_w = v;
            }
        }
        let cr_out = cr_tl.step(frame.delta_time);
        if let Some(track_vals) = cr_out.first() {
            if let Some(&v) = track_vals.last() {
                current_cr = v;
            }
        }

        // Decay pulse timer
        let t = pulse_timer.get();
        if t > 0.0 {
            let nt = (t - frame.delta_time).max(0.0);
            pulse_timer.set(nt);
        }
        let pulse_active = pulse_timer.get() > 0.0;

        // Draw nine-slice panel and animated focus ring
        if let Some(aid) = atlas_id {
            let nine = NinePatch::new(aid, tile, insets);
            let rect = Rectangle::new(current_pos.x, current_pos.y, current_w, 120.0);
            nine.draw(engine, rect, 0);
            let ring_color = if pulse_active {
                [1.0, 0.4, 0.4, 1.0]
            } else {
                [1.0, 0.9, 0.2, 1.0]
            };
            draw_focus_ring(
                engine,
                Rectangle::new(
                    current_pos.x - 4.0,
                    current_pos.y - 4.0,
                    current_w + 8.0,
                    120.0 + 8.0,
                ),
                FocusRingStyle {
                    thickness_px: 2.0,
                    color: ring_color,
                    corner_radius_px: current_cr,
                    inset_px: 0.0,
                },
                1,
            );
        }

        engine.end_frame().unwrap();
    })?;

    Ok(())
}
