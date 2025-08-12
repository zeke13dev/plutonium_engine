use plutonium_engine::texture_atlas::TextureAtlas;
use plutonium_engine::utils::Size;

#[test]
fn uv_for_first_tile_is_in_top_left_quadrant() {
    let atlas_size = Size {
        width: 256.0,
        height: 256.0,
    };
    let tile_size = Size {
        width: 128.0,
        height: 128.0,
    };
    let uv = TextureAtlas::tile_uv_coordinates(0, tile_size, atlas_size).unwrap();
    assert!(
        uv.x >= 0.0 && uv.y >= 0.0,
        "uv offset should be non-negative"
    );
    assert!(
        uv.width > 0.0 && uv.height > 0.0,
        "uv size should be positive"
    );
    assert!(
        uv.x + uv.width <= 1.0 && uv.y + uv.height <= 1.0,
        "uv should be in [0,1]"
    );
}

#[test]
fn uv_for_second_tile_moves_right() {
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
    assert!(
        right.x > left.x,
        "next tile in row should have larger u offset"
    );
}
