#[cfg(feature = "anim")]
mod anim_tests {
    use plutonium_engine::anim::{ease_value, Ease, Track, Tween};

    #[test]
    fn easing_curves_basic() {
        assert!((ease_value(Ease::Linear, 0.0) - 0.0).abs() < 1e-6);
        assert!((ease_value(Ease::Linear, 1.0) - 1.0).abs() < 1e-6);
        let e_in = ease_value(Ease::EaseIn, 0.5);
        let e_out = ease_value(Ease::EaseOut, 0.5);
        assert!(e_in < 0.5 && e_out > 0.5);
        // Cubic bezier monotonicity and endpoints for a common ease curve
        let bez = Ease::CubicBezier {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        };
        let v25 = ease_value(bez, 0.25);
        let v50 = ease_value(bez, 0.5);
        let v75 = ease_value(bez, 0.75);
        assert!(v25 <= v50 && v50 <= v75);
        assert!((ease_value(bez, 0.0) - 0.0).abs() < 1e-6);
        assert!((ease_value(bez, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn tween_steps_numeric() {
        let mut tw = Tween::new(0.0f32, 10.0f32, 1.0, Ease::Linear);
        let v0 = tw.sample();
        assert!((v0 - 0.0).abs() < 1e-6);
        let v1 = tw.step(0.5);
        assert!((v1 - 5.0).abs() < 1e-4);
        let v2 = tw.step(0.5);
        assert!((v2 - 10.0).abs() < 1e-4);
        assert!(tw.is_finished());
    }

    #[test]
    fn track_sequence_consumes_dt() {
        let mut seq = Track::Sequence(vec![
            Tween::new(0.0f32, 1.0f32, 0.25, Ease::Linear),
            Tween::new(1.0f32, 2.0f32, 0.25, Ease::Linear),
        ]);
        // Step with dt larger than first tween; should advance into second
        let out = seq.step(0.3);
        // At least one value produced
        assert!(!out.is_empty());
    }
}

#[cfg(feature = "anim")]
mod timeline_tests {
    use plutonium_engine::anim::{Ease, Timeline, Track, Tween};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn timeline_callbacks_fire() {
        let mut tl: Timeline<f32> = Timeline::new();
        tl.push_track(Track::Sequence(vec![Tween::new(
            0.0,
            1.0,
            0.5,
            Ease::Linear,
        )]));
        tl.add_label("half", 0.25);
        let fired = Rc::new(Cell::new(false));
        let fired_closure = Rc::clone(&fired);
        tl.on_label("half", move || fired_closure.set(true));
        let _ = tl.step(0.2);
        assert!(!fired.get());
        let _ = tl.step(0.1);
        assert!(fired.get());
    }

    #[test]
    fn timeline_seek_and_rate() {
        let mut tl: Timeline<f32> = Timeline::new();
        tl.push_track(Track::Sequence(vec![Tween::new(
            0.0,
            1.0,
            1.0,
            Ease::Linear,
        )]));
        let mut values = Vec::new();
        // Step a bit, record
        for _ in 0..5 {
            let out = tl.step(0.1);
            if let Some(v) = out.first().and_then(|v| v.first()).copied() {
                values.push(v);
            }
        }
        assert!(values.windows(2).all(|w| w[1] >= w[0]));
        // Seek to mid and verify we progressed
        tl.seek(0.5);
        let after_seek = tl.step(0.0); // no-op step to sample
        assert!(after_seek.first().map(|v| !v.is_empty()).unwrap_or(false));
        // Change rate and advance; should move faster
        tl.set_rate(2.0);
        let v1 = tl.step(0.1); // equals 0.2 of progress
        let v2 = tl.step(0.1);
        let a = v1.first().and_then(|v| v.first()).copied().unwrap_or(0.0);
        let b = v2.first().and_then(|v| v.first()).copied().unwrap_or(0.0);
        assert!(b >= a);
    }
}
