struct TransformUniform {
    transform: mat4x4<f32>,
};

@group(1) @binding(0)
var<uniform> transformUniform: TransformUniform;

struct InstanceData {
    model: mat4x4<f32>,
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,
    msdf_px_range: f32,
    _msdf_pad: array<f32, 3>,
    tint: vec4<f32>,
};

@group(3) @binding(0)
var<storage, read> instances: array<InstanceData>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) uv_offset: vec2<f32>,
    @location(2) uv_scale: vec2<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) msdf_px_range: f32,
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
    output.tint = inst.tint;
    output.msdf_px_range = inst.msdf_px_range;
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

fn median3(v: vec3<f32>) -> f32 {
    return max(min(v.x, v.y), min(max(v.x, v.y), v.z));
}

@fragment
fn fs_main(
    @location(0) tex_coords: vec2<f32>,
    @location(1) uv_offset: vec2<f32>,
    @location(2) uv_scale: vec2<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) msdf_px_range: f32,
) -> @location(0) vec4<f32> {
    let adjusted_tex_coords = tex_coords * uv_scale + uv_offset;
    // MSDF data must be sampled from base mip only; mips corrupt encoded distances.
    let sample_rgba = textureSampleLevel(my_texture, my_sampler, adjusted_tex_coords, 0.0);
    let msdf_sd = median3(sample_rgba.rgb) - 0.5;
    let sdf_sd = sample_rgba.a - 0.5;

    let px_range = max(msdf_px_range, 1.0);
    // Clamp MSDF to monochrome SDF neighborhood to suppress corner artifacts.
    let clamp_width = 1.0 / px_range;
    let msdf_clamped = clamp(msdf_sd, sdf_sd - clamp_width, sdf_sd + clamp_width);
    // Use clamped MSDF distance directly; alpha-SDF envelope min-clamping can
    // bias glyph sidebearings in this generator and create pair-specific
    // visual spacing drift (e.g. `j/u` vs `r/t`).
    let sd = msdf_clamped;
    let atlas_dims = vec2<f32>(textureDimensions(my_texture));
    let unit_range = vec2<f32>(px_range, px_range) / max(atlas_dims, vec2<f32>(1.0, 1.0));
    let screen_tex_size = vec2<f32>(1.0, 1.0) / max(fwidth(adjusted_tex_coords), vec2<f32>(1e-6, 1e-6));
    let screen_px_range = max(0.5 * dot(unit_range, screen_tex_size), 1.0);
    let alpha = clamp(screen_px_range * sd + 0.5, 0.0, 1.0);

    let out_tint = tint * uvTransform.tint;
    return vec4<f32>(out_tint.rgb, out_tint.a * alpha);
}
