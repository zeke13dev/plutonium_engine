# Scenes and Focus

## Scenes
- `SceneStack`: push/pop/replace; emits `SceneEnter`/`SceneExit` events.
- `SceneSystems`: register per-scene `startup`, `update`, `render` schedules and run them based on the current scene.

## Focus
- `FocusManager`: tracks focused widget index; `next`/`prev`; demo uses Tab to cycle and Enter/Space to activate.
- Keyboard support: arrows adjust Slider.

See `examples/demo_card_game` for usage.
