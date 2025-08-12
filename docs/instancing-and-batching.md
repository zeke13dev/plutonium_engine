## Instancing and batching

Plutonium now performs true instanced rendering for single-texture sprites.

- What changed:
  - The render queue stores indices into a per-frame transform pool rather than individual bind groups per sprite.
  - During submission, items are grouped by `texture_key`, their 4x4 transforms are packed into a storage buffer, and a single `draw_indexed` is issued with `instance_count = N`.
  - The WGSL vertex shader reads `@builtin(instance_index)` and fetches `instanceTransforms.transforms[instance_index]` from `@group(3)`.
  - The world/camera transform remains a uniform at `@group(1)`.

- Pipeline/bindings:
  - Group(0): texture+sampler
  - Group(1): world/camera `TransformUniform`
  - Group(2): UV transform (for atlases)
  - Group(3): storage buffer of per-instance `mat4x4<f32>`

- Performance tips:
  - Submit sprites using the immediate-mode API `draw_texture` or widget `render`; the engine batches by texture automatically.
  - Prefer larger batches with fewer textures to reduce bind group switches.
  - Keep instance buffer sizes reasonable per batch; thousands of instances per draw are fine on desktop.

- Compatibility:
  - Atlas tiles are still drawn non-instanced for now; future versions will add atlas instancing.
  - Snapshots create a single-instance identity bind group to match the pipeline layout.


