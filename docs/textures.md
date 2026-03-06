# Textures

Plutonium Engine supports SVG and Raster (PNG/JPEG) textures.

## Loading Textures

### SVG Textures
SVG is the primary format for Plutonium Engine.
```rust
let (id, dims) = engine.create_texture_svg("path/to/image.svg", Position { x: 0.0, y: 0.0 }, 1.0);
```

### Raster Textures
Requires the `raster` feature.
```rust
let (id, dims) = engine.create_texture_raster_from_path("path/to/image.png", Position { x: 0.0, y: 0.0 });
```

On `wasm32`, raster textures can also be loaded from runtime URLs:
```rust
let (id, size) = engine
    .create_texture_raster_from_url("/assets/blue.png", Position { x: 0.0, y: 0.0 })
    .await?;
```

For synchronous frame loops on `wasm32`, use the pollable path:
```rust
let handle = engine.begin_texture_raster_from_url("/assets/blue.png", Position { x: 0.0, y: 0.0 });
if let Some(result) = engine.poll_texture_raster_from_url(handle) {
    let (id, size) = result?;
}
```

## Drawing Textures

### Normal Drawing
Draws a texture at a specific position with optional scaling, rotation, and tint.
```rust
engine.draw_texture(
    &texture_id,
    Position { x: 100.0, y: 100.0 },
    DrawParams {
        z: 0,
        scale: 1.0,
        rotation: 0.0,
        tint: [1.0, 1.0, 1.0, 1.0],
    },
);
```

### Stretched Drawing (Responsive Fit)
Draws a texture into a target rectangle with various fitting modes.

```rust
let target_rect = Rectangle::new(100.0, 100.0, 400.0, 300.0);
engine.draw_texture_stretched_with_fit_and_inset(
    &texture_id,
    target_rect,
    TextureFit::Contain,
    10.0, // logical inset (padding)
    0,    // z-layer
);
```

#### TextureFit Modes
- `TextureFit::StretchFill`: Stretches the texture to exactly fill the destination rectangle. Does NOT preserve aspect ratio.
- `TextureFit::Contain`: Scales the texture to fit inside the rectangle while preserving its aspect ratio. Results in letterboxing if aspect ratios don't match.
- `TextureFit::Cover`: Scales the texture to completely fill the rectangle while preserving its aspect ratio. Excess is cropped.

### Logical Insets
The `inset` parameter (available in stretched draw calls) allows you to specify a uniform logical padding around the texture within the target rectangle.

## NDC Space & Units
Plutonium Engine uses a 0.0 to 1.0 unit quad for all texture rendering. 
- **Logical Pixels**: All API positions and sizes are in logical pixels.
- **DPI Awareness**: The engine automatically handles DPI scaling internally.
- **Aspect Ratio**: Aspect ratios are calculated using the original pixel dimensions of the source asset to ensure correctness across different display scales.
