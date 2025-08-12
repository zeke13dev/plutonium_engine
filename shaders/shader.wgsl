struct TransformUniform {
    transform: mat4x4<f32>,
};

@group(1) @binding(0)
var<uniform> transformUniform: TransformUniform;

// Per-instance data for true instancing
struct InstanceData {
    model: mat4x4<f32>,
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,
};

@group(3) @binding(0)
var<storage, read> instances: array<InstanceData>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) uv_offset: vec2<f32>,
    @location(2) uv_scale: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var output: VertexOutput;
    let inst = instances[instance_index];
    output.position = transformUniform.transform * inst.model * vec4<f32>(position, 0.0, 1.0);

    output.tex_coords = tex_coords;
    output.uv_offset = inst.uv_offset;
    output.uv_scale = inst.uv_scale;
    return output;
}

@group(0) @binding(0)
var my_texture: texture_2d<f32>;

@group(0) @binding(1)
var my_sampler: sampler;

struct UVTransform {
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,
    tint: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> uvTransform: UVTransform;

@fragment
fn fs_main(
    @location(0) tex_coords: vec2<f32>,
    @location(1) uv_offset: vec2<f32>,
    @location(2) uv_scale: vec2<f32>,
) -> @location(0) vec4<f32> {
    let adjustedTexCoords = tex_coords * uv_scale + uv_offset;
    let base = textureSample(my_texture, my_sampler, adjustedTexCoords);
    // The bound UVTransform (group 2) carries tint; uv_offset/scale come per-instance
    return base * uvTransform.tint;
}
