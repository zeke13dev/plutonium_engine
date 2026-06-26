use plutonium_engine::texture_atlas::TextureAtlas;
use plutonium_engine::utils::{Position, Size};

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

fn assert_matrix_entry(matrix: [[f32; 4]; 4], row: usize, col: usize, expected: f32) {
    assert!(
        approx_eq(matrix[row][col], expected),
        "matrix[{row}][{col}] expected {expected}, got {}",
        matrix[row][col]
    );
}

#[test]
fn transform_uniform_maps_full_viewport_tile_to_ndc() {
    let viewport = Size {
        width: 200.0,
        height: 100.0,
    };
    let tile = viewport;

    let tf = TextureAtlas::compute_transform_matrix(
        viewport,
        Position { x: 0.0, y: 0.0 },
        Position { x: 0.0, y: 0.0 },
        1.0,
        tile,
    );

    assert_matrix_entry(tf, 0, 0, 2.0);
    assert_matrix_entry(tf, 1, 1, -2.0);
    assert_matrix_entry(tf, 3, 0, 0.0);
    assert_matrix_entry(tf, 3, 1, 0.0);
}

#[test]
fn transform_uniform_centers_half_viewport_tile_exactly() {
    let viewport = Size {
        width: 200.0,
        height: 100.0,
    };
    let tile = Size {
        width: 100.0,
        height: 50.0,
    };

    let tf = TextureAtlas::compute_transform_matrix(
        viewport,
        Position { x: 50.0, y: 25.0 },
        Position { x: 0.0, y: 0.0 },
        1.0,
        tile,
    );

    assert_matrix_entry(tf, 0, 0, 1.0);
    assert_matrix_entry(tf, 1, 1, -1.0);
    assert_matrix_entry(tf, 3, 0, 0.0);
    assert_matrix_entry(tf, 3, 1, 0.0);
}

#[test]
fn transform_uniform_preserves_top_left_y_down_polarity() {
    let viewport = Size {
        width: 200.0,
        height: 100.0,
    };
    let tile = Size {
        width: 20.0,
        height: 10.0,
    };

    let top = TextureAtlas::compute_transform_matrix(
        viewport,
        Position { x: 0.0, y: 0.0 },
        Position { x: 0.0, y: 0.0 },
        1.0,
        tile,
    );
    let lower = TextureAtlas::compute_transform_matrix(
        viewport,
        Position { x: 0.0, y: 10.0 },
        Position { x: 0.0, y: 0.0 },
        1.0,
        tile,
    );

    assert_matrix_entry(top, 3, 1, 0.9);
    assert_matrix_entry(lower, 3, 1, 0.7);
    assert!(
        lower[3][1] < top[3][1],
        "positive logical y should move down in screen space and decrease NDC y"
    );
}
