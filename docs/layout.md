# Layout (feature: `layout`)

Minimal, optional helpers for positioning UI or sprites in logical space.

- Anchors: horizontal (Left, Center, Right), vertical (Top, Middle, Bottom)
- Percent sizing: specify width/height as a fraction of container
- Margins: left/right/top/bottom

API:
- `layout_node(container: Rectangle, desired: Size, params: LayoutParams) -> LayoutResult`
  - `container`: area to layout into (e.g., window bounds)
  - `desired`: fallback size if percent is not set
  - `params`: anchors, percent sizing, margins
  - returns position and size in logical pixels

Notes:
- Camera transforms are applied after layout; layout runs in logical screen space.
- This is intentionally minimal. More advanced layouts (flex/grid/constraints) belong in the higher-level game engine.

How to place HUD elements (example):

1. Define your container as the window bounds `Rectangle::new(0.0, 0.0, width, height)`.
2. Choose anchors and optional percent size. For a centered HUD element at 40% width and 10% height:
   - `anchors: Anchors { h: HAnchor::Center, v: VAnchor::Middle }`
   - `percent: Some(PercentSize { width_pct: 0.4, height_pct: 0.1 })`
3. Call `layout_node` to obtain `position` and `size`, then pass `position` to `draw_texture` or your widget's `.set_pos()`.
4. HUD coordinates are logical and unaffected by DPI; the engine applies DPI and camera after layout.
