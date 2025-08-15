# Plutonium Engine – Product Requirements Document (PRD)

## Vision
Deliver a focused 2D engine optimized to ship Balatro‑class games: crisp UI, deterministic animation, robust input, and snapshot‑driven testing. Keep the core lean, fast, and modular, with feature flags to scale from minimal builds to a fully tooled workflow.

## What’s shipped so far
- Feature flag for dev assets hot‑reload: `dev-hot-reload` (off by default in release).
- UI primitives foundation:
  - Rect SDF pipeline with rounded corners and optional border.
  - Focus ring utility.
  - Nine‑slice drawing helper (`ui::NinePatch`) with stretchable center/edges.
  - Headless snapshots for menu panel/button and focused state.
- Engine capability snapshots added:
  - Transitions multi‑frame: `snapshots/actual/transitions.png` and `transitions_frame2.png`.
  - Deal layout: `snapshots/actual/deal_grid.png`.
  - Menu panel and button: `snapshots/actual/menu_panel.png`, `menu_button.png`.

## Goals (engine scope)
- Rendering/UI: batched rect primitives, nine‑slice scaling (no seams), offscreen targets; pixel‑perfect results at any DPI.
- Animation: tween chains and parallel groups with easing curves, delays, and a seekable timeline with labels/callbacks.
- Input: keyboard/mouse/controller with action bindings; IME text composition, key repeat, clipboard.
- Layout: anchors/constraints, min/max, and simple flex‑like containers; deterministic evaluation independent of camera.
- Data‑driven scenes: YAML/TOML descriptions for widgets, actions, and transitions.
- Save/replay: seeded RNG streams and an input/event log; fast‑forward/step tools.
- Debug HUD: draw calls, batches, GPU/frame time; input inspector; overlay toggles.
- Audio: feature‑gated mixer with groups, fades, ducking.

## Non‑goals (v1)
- Networked multiplayer or rollback netcode.
- Heavy in‑editor tooling or visual node graphs.
- Complex physics or 3D rendering.

## Key design principles
- Determinism first: fixed‑dt stepping path, seeded RNG streams, and snapshot tests.
- Batch everything: minimize state changes; prefer vector/rect primitives for UI over textures.
- Ergonomic API: small, composable types; clear feature flags; examples and tests as ground truth.

## First capability tranche (recommended now)
Ship these together to unlock dealing animations, UI polish, and deterministic testing.

1) UI primitives (feature: `ui-primitives`)
   - Rect batching API with border/corner radius and pixel‑alignment. [in progress]
   - Nine‑slice drawing (stretch center spans; seams eliminated via pixel‑snapped transforms). [done]
   - Focus ring drawing utility (thickness, color, corner radius, inset). [done]
   - Snapshot scenes: `menu_panel.png`, `menu_button.png` (+ focused state), slider with focus ring. [partially done]

2) Tweening (feature: `anim`)
   - `Tween<T>` with easing curves (Linear, EaseIn/Out/InOut) working for scalars and Position; CSS‑like cubic‑bezier implemented.
   - `Sequence` and `Parallel` combinators; `Timeline` with labels/callbacks; `seek`, `set_rate`; fixed‑dt integration with app tick. [in progress]
   - Tests covering numeric outputs and deterministic stepping; drive multi‑frame snapshots (e.g., `transitions_frame2.png`). [basic tests done]

3) RNG + replay (feature: `replay`)
   - `RngService` with named streams (e.g., `Deal`, `Shuffle`, `Particles`). [done]
   - Minimal record/replay: frame‑stamped input log plus seeds/window; CLI flags in snapshot runner `--seed`, `--record`, `--replay`. [in progress]
   - Determinism tests: same seed → identical snapshots and deck order. [planned]

## API sketch (non‑binding)
- Rendering primitives
  - `renderer::RectCommand { rect, color, corner_radius, border: Option<BorderSpec> }`
  - `renderer::RectBatch::draw(&mut self, &[RectCommand])`
  - `ui::NinePatch { texture, insets, mode }` and `draw_nine_patch(...)`
  - `ui::FocusRingStyle { thickness_px, color, corner_radius, inset_px }` and `draw_focus_ring(...)`
- Anim/tween
  - `anim::{Tween<T>, Sequence, Parallel, Ease}` and `Timeline`
- RNG/replay
  - `core::rng::{RngService, RngStreamId}`; snapshot CLI `--seed`, `--record`, `--replay`

## Feature flags
- `dev-hot-reload` (existing)
- `ui-primitives`
- `anim` (added)
- `replay` (added)

Default: examples opt‑in to all; library default keeps nonessential features off.

## Testing strategy
- Golden image snapshots for panels/buttons/focus state/slider and transitions frames.
- Unit tests for easing functions (done), upcoming: tween sequencing, RNG determinism, and record/replay round‑trip.
 
## Runtime controls
- Fixed‑dt stepping toggle via `PlutoniumApp::set_fixed_timestep(dt_seconds)` to ensure deterministic updates.

## Milestones (high level)
1. UI primitives (rect batch, nine‑slice, focus ring) + snapshots.
2. Tweening core + timeline + deterministic stepping.
3. RNG streams + minimal record/replay + determinism tests.
4. Input/IME (text composition, key repeat, clipboard) + controller action bindings.
5. Layout anchors/constraints, min/max, simple flex‑like containers.
6. Offscreen render targets; HUD metrics (draw calls, batches, GPU/frame time).
7. Data‑driven scenes YAML/TOML.
8. Audio mixer (feature‑gated) with groups, fades, ducking.


