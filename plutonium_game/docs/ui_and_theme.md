# UI and Theme

## Theme
- Colors: primary_text_rgba, button_bg_rgba, button_bg_hover_rgba.
- Keep minimal; extend with typography and spacing later.
- Register Theme as a world resource for systems to read.

## Widgets
- Label: draws text via RenderCommands.
- Button: hit-testing via InputState; optional background sprite with tint from Theme.

## Render submission
- Submit RenderCommands each frame: sprites use per-command tint; texts use engine queue_text.
