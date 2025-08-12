# Coordinates and DPI

- Logical pixels; origin at top-left, +x right, +y down.
- DPI scaling handled inside the engine. Provide logical positions; the engine applies device scale.
- Camera operates in logical space. UI layout should run before camera transforms.
