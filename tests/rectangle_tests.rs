use plutonium_engine::utils::Rectangle;

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn assert_rect_eq(actual: Rectangle, expected: Rectangle) {
    assert!(
        approx_eq(actual.x, expected.x),
        "x: {} != {}",
        actual.x,
        expected.x
    );
    assert!(
        approx_eq(actual.y, expected.y),
        "y: {} != {}",
        actual.y,
        expected.y
    );
    assert!(
        approx_eq(actual.width, expected.width),
        "width: {} != {}",
        actual.width,
        expected.width
    );
    assert!(
        approx_eq(actual.height, expected.height),
        "height: {} != {}",
        actual.height,
        expected.height
    );
}

#[test]
fn rectangle_pad_expands_symmetrically() {
    let rect = Rectangle::new(10.0, 20.0, 30.0, 40.0);

    assert_rect_eq(
        Rectangle::pad(&rect, 5.0),
        Rectangle::new(5.0, 15.0, 40.0, 50.0),
    );
}

#[test]
fn rectangle_pad_negative_padding_shrinks_symmetrically() {
    let rect = Rectangle::new(10.0, 20.0, 30.0, 40.0);

    assert_rect_eq(
        Rectangle::pad(&rect, -5.0),
        Rectangle::new(15.0, 25.0, 20.0, 30.0),
    );
}
