use plutonium_engine::{HaloFalloff, HaloPreset, HaloStyle};

#[test]
fn halo_falloff_curves_fade_outward() {
    let curves = [
        HaloFalloff::Linear,
        HaloFalloff::EaseOut,
        HaloFalloff::Smoothstep,
        HaloFalloff::Exponential(2.0),
        HaloFalloff::InverseSquare,
    ];

    for curve in curves {
        let near = curve.sample(0.1);
        let mid = curve.sample(0.5);
        let far = curve.sample(0.9);
        assert!(near >= mid, "near alpha should be >= mid alpha");
        assert!(mid >= far, "mid alpha should be >= far alpha");
    }
}

#[test]
fn halo_style_alpha_respects_max_alpha() {
    let style = HaloStyle {
        color: [1.0, 1.0, 1.0, 1.0],
        intensity: 5.0,
        max_alpha: 0.35,
        ..HaloStyle::default()
    };

    let alpha = style.alpha_at(0.0);
    assert!(alpha <= 0.35 + f32::EPSILON);
}

#[test]
fn halo_style_pulse_affects_alpha_when_enabled() {
    let base = HaloStyle {
        pulse_amplitude: 0.0,
        pulse_speed_hz: 0.0,
        ..HaloStyle::default()
    };
    let pulsed = HaloStyle {
        pulse_amplitude: 0.5,
        pulse_speed_hz: 2.0,
        time_seconds: 0.125, // quarter cycle at 2Hz
        ..base
    };

    let base_alpha = base.alpha_at(0.2);
    let pulsed_alpha = pulsed.alpha_at(0.2);
    assert!(pulsed_alpha >= base_alpha);
}

#[test]
fn halo_from_preset_produces_expected_characteristics() {
    let primary = HaloStyle::from_preset(HaloPreset::TutorialPrimary);
    let subtle = HaloStyle::from_preset(HaloPreset::TutorialSubtle);
    let urgent = HaloStyle::from_preset(HaloPreset::TutorialUrgent);

    assert!(subtle.max_alpha < primary.max_alpha);
    assert!(urgent.max_alpha > primary.max_alpha);
    assert!(urgent.radius > primary.radius);
    assert!(subtle.radius < urgent.radius);
}
