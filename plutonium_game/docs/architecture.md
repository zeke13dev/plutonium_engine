# Architecture

- Renderer stays ECS-agnostic (immediate-mode draw API in plutonium_engine).
- Game crates: core (ECS/schedules/time/events/RNG), input, assets, ui, audio, gameplay.
- Examples: demo_card_game wires a window loop and calls into schedules.

Key boundaries:
- Render systems build RenderCommands; a submission function calls engine draws.
- Assets resolve to engine handles (textures/fonts) at render time.
- Input collects raw events → actions/edges.

Schedules:
- startup: one-time world setup.
- fixed_update: deterministic step (accumulator in app loop).
- update: variable per-frame logic.
- render: optional systems preparing draw commands.
