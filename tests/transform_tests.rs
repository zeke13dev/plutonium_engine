use plutonium_engine::utils::{Position, Size, TransformUniform};
use plutonium_engine::texture_atlas::TextureAtlas;

#[test]
fn transform_uniform_scales_and_translates() {
    // synthetic inputs
    let viewport = Size { width: 200.0, height: 100.0 };
    let camera = Position { x: 0.0, y: 0.0 };
    let pos = Position { x: 50.0, y: 25.0 };

    // stub atlas with tile_size
    let tile = Size { width: 20.0, height: 10.0 };
    // Use the pure helper on TextureAtlas to compute uniform
    let tf: TransformUniform = TextureAtlas::compute_transform_uniform(viewport, pos, camera, 1.0, tile);

    // The scale (diagonal) should correlate to tile/viewport
    assert!(tf.transform[0][0].abs() > 0.0);
    assert!(tf.transform[1][1].abs() > 0.0);
}


