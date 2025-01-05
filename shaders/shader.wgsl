struct TransformUniform {
    transform: mat4x4<f32>,
};

@group(1) @binding(0)
var<uniform> transformUniform: TransformUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) tex_coords: vec2<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.position = transformUniform.transform * vec4<f32>(position, 0.0, 1.0);

    output.tex_coords = tex_coords;
    return output;
}

@group(0) @binding(0)
var my_texture: texture_2d<f32>;

@group(0) @binding(1)
var my_sampler: sampler;

struct UVTransform {
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,
};

@group(2) @binding(0)
var<uniform> uvTransform: UVTransform;

@fragment
fn fs_main(@location(0) tex_coords: vec2<f32>) -> @location(0) vec4<f32> {
    let adjustedTexCoords = tex_coords * uvTransform.uv_scale + uvTransform.uv_offset;
    return textureSample(my_texture, my_sampler, adjustedTexCoords);
}
