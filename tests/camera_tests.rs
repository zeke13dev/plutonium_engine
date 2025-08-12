use plutonium_engine::camera::Camera;
use plutonium_engine::utils::{Position, Rectangle};

#[test]
fn camera_stays_within_boundary() {
    let mut cam = Camera::new(Position { x: 0.0, y: 0.0 });
    cam.set_boundary(Rectangle::new(0.0, 0.0, 100.0, 100.0));
    cam.activate();
    cam.set_pos(Position { x: 150.0, y: 150.0 });
    let pos = cam.get_pos(1.0);
    assert!(pos.x >= 50.0 && pos.y >= 50.0);
}


