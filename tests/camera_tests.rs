use plutonium_engine::camera::Camera;
use plutonium_engine::utils::{Position, Rectangle};

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn camera_get_pos_is_zero_when_deactivated_and_scaled_when_active() {
    let mut cam = Camera::new(Position { x: 0.0, y: 0.0 });
    cam.set_pos(Position { x: 10.0, y: 20.0 });

    let inactive = cam.get_pos(2.0);
    assert!(approx_eq(inactive.x, 0.0));
    assert!(approx_eq(inactive.y, 0.0));

    cam.activate();
    let active = cam.get_pos(2.0);
    assert!(approx_eq(active.x, 20.0));
    assert!(approx_eq(active.y, 40.0));
}

#[test]
fn camera_deadzone_only_moves_on_boundary_overflow() {
    let mut cam = Camera::new(Position { x: 0.0, y: 0.0 });
    cam.activate();
    cam.set_boundary(Rectangle::new(0.0, 0.0, 100.0, 100.0));

    // Inside deadzone: no camera movement.
    cam.set_pos(Position { x: 50.0, y: 50.0 });
    let pos = cam.logical_pos();
    assert!(approx_eq(pos.x, 0.0));
    assert!(approx_eq(pos.y, 0.0));

    // Overflow right by 20px: camera should move right by 20.
    cam.set_pos(Position { x: 120.0, y: 50.0 });
    let pos = cam.logical_pos();
    assert!(approx_eq(pos.x, 20.0));
    assert!(approx_eq(pos.y, 0.0));

    // With camera at x=20, deadzone left edge is x=20. Target x=10 overflows left by -10.
    cam.set_pos(Position { x: 10.0, y: 50.0 });
    let pos = cam.logical_pos();
    assert!(approx_eq(pos.x, 10.0));
    assert!(approx_eq(pos.y, 0.0));
}

#[test]
fn camera_smoothing_converges_toward_target_without_overshoot() {
    let mut cam = Camera::new(Position { x: 0.0, y: 0.0 });
    cam.activate();
    cam.set_smoothing_strength(20.0);

    let target = Position { x: 100.0, y: 0.0 };
    let mut previous = cam.logical_pos().x;
    for _ in 0..30 {
        cam.set_pos_with_dt(target, 1.0 / 60.0);
        let current = cam.logical_pos().x;
        assert!(
            current >= previous,
            "smoothed camera should move monotonically"
        );
        assert!(current <= target.x, "smoothed camera should not overshoot");
        previous = current;
    }

    assert!(
        cam.logical_pos().x > 99.9,
        "camera should converge close to the target after enough steps"
    );
}

#[test]
fn camera_set_pos_with_dt_is_immediate_when_smoothing_is_disabled() {
    let mut cam = Camera::new(Position { x: 0.0, y: 0.0 });
    cam.activate();
    cam.set_smoothing_strength(0.0);

    cam.set_pos_with_dt(Position { x: 77.0, y: 33.0 }, 1.0 / 60.0);
    let pos = cam.logical_pos();
    assert!(approx_eq(pos.x, 77.0));
    assert!(approx_eq(pos.y, 33.0));
}
