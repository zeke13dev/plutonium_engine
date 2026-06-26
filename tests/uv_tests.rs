use plutonium_engine::texture_atlas::TextureAtlas;
use plutonium_engine::utils::Size;

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn uv_for_first_tile_has_exact_half_texel_inset() {
    let atlas_size = Size {
        width: 256.0,
        height: 256.0,
    };
    let tile_size = Size {
        width: 128.0,
        height: 128.0,
    };
    let uv = TextureAtlas::tile_uv_coordinates(0, tile_size, atlas_size).unwrap();

    assert!(approx_eq(uv.x, 0.5 / 256.0));
    assert!(approx_eq(uv.y, 0.5 / 256.0));
    assert!(approx_eq(uv.width, 127.0 / 256.0));
    assert!(approx_eq(uv.height, 127.0 / 256.0));
}

#[test]
fn uv_for_second_tile_moves_right_by_one_tile() {
    let atlas_size = Size {
        width: 256.0,
        height: 256.0,
    };
    let tile_size = Size {
        width: 128.0,
        height: 128.0,
    };
    let left = TextureAtlas::tile_uv_coordinates(0, tile_size, atlas_size).unwrap();
    let right = TextureAtlas::tile_uv_coordinates(1, tile_size, atlas_size).unwrap();

    assert!(approx_eq(right.x - left.x, 128.0 / 256.0));
    assert!(approx_eq(right.y, left.y));
    assert!(approx_eq(right.width, left.width));
    assert!(approx_eq(right.height, left.height));
}

#[test]
fn uv_for_first_tile_on_second_row_moves_down_by_one_tile() {
    let atlas_size = Size {
        width: 256.0,
        height: 256.0,
    };
    let tile_size = Size {
        width: 128.0,
        height: 128.0,
    };
    let top_left = TextureAtlas::tile_uv_coordinates(0, tile_size, atlas_size).unwrap();
    let bottom_left = TextureAtlas::tile_uv_coordinates(2, tile_size, atlas_size).unwrap();

    assert!(approx_eq(bottom_left.x, top_left.x));
    assert!(approx_eq(bottom_left.y - top_left.y, 128.0 / 256.0));
    assert!(approx_eq(bottom_left.width, top_left.width));
    assert!(approx_eq(bottom_left.height, top_left.height));
}
