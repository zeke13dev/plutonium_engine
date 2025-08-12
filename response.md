once again feel free to challenge me, push me, teach me, question me


Questions to refine API and roadmap:
- Universal coordinates: prefer origin top-left, +x right, +y down, in logical pixels?
Any target logical resolution or just window-size logical units?


for the coordinate system i'm not quite sure because i see the need for game-like coordinates and also relative coordinates.
i'm not sure if we should have the game-like coordinates for the game engine module or not.
i want to have a somewhat innovative and thoughtful system for positioning. kind of like. a more configurable nextjs in a way.
by this i mean that i want it to be a pipeline where the user will say i always want this item to be towards the center
or left, or aligned with this item. i think relative to other blocks and relative to screen is good but i want it to build
off of tried and tested HTML/CSS methods. i want this to be a little fresh. let's talk about it


- Dual API naming: immediate-mode calls like `draw_texture`, `draw_text`, `draw_rect`, and retained `Texture2D`, `Text2D`
objects acceptable? Or prefer `begin_frame/end_frame` with a `Frame` builder context (e.g., `frame.draw_texture(...)`)?

originally i was thinking that it makes sense to have the abstracted away objects, but then i started to ponder if those
objects belonged in the game engine and not the graphics engine, so i'm kind of conflicted help me decide.

- Backend swap: fine to define a `Renderer` trait (create texture, queue draws, submit), and make `PlutoniumEngine` depend on that
trait? This gives us a clean seam to replace WGPU code later.

i'm not quite sure what this entails but i think it's okay

- Layers: do you want simple integer `z` layers or named layers (e.g., “world”, “ui”) with an order map?

is there a way to let the user create that as some kind of enum order map or have that as a later feature in the game engine?
i think for the graphics engine side it makes sense to just use the traditional z layers though. thoughts?

Next concrete steps I propose:
- Implement a logical coordinate system (documented, DPI-safe) and centralize conversions.
- Add layers (`z: i32`) and sorting in `render_queue`.
- Introduce `begin_frame()/end_frame()` and a simple immediate-mode draw API.
- Add PNG/JPEG loader path and a unified `TextureSource` enum (SVG, Raster).
- Start a `Renderer` trait and move wgpu specifics behind it (no behavior change).
- Write a text-input example that confirms typing/editing works via the default app loop.

Let me know your preferences on:
- Coordinate origin/conventions and whether to expose a configurable virtual resolution.
- Immediate-mode API style: free functions on `PlutoniumEngine` vs a `Frame` handle.
- Layer model (int vs named).
- Proceed now with the above edits?

- Passed keys into `engine.update` in `src/app.rs` so widgets receive input.
- Replaced mouse DPI hack with `mouse_pos / self.dpi_scale_factor` in `src/lib.rs`.
- Handled `wgpu::SurfaceError` cases in `render()` for resilience.
- Pruned unused dependencies in `Cargo.toml`.
- Build check succeeded; only benign warnings remain.
