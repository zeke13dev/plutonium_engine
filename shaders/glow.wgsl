struct TransformUniform {
    transform: mat4x4<f32>,
};

@group(1) @binding(0)
var<uniform> transformUniform: TransformUniform;

struct GlowInstanceData {
    model: mat4x4<f32>,
    color: vec4<f32>,
    rect_size_px: vec2<f32>,
    corner_radius_px: f32,
    glow_radius_px: f32,
    sigma: f32,
    max_alpha: f32,
    mode: f32,
    border_width: f32,
    _pad: vec4<f32>,
};

@group(3) @binding(0)
var<storage, read> instances: array<GlowInstanceData>;

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos_ndc: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) rect_size_px: vec2<f32>,
    @location(3) corner_radius_px: f32,
    @location(4) glow_radius_px: f32,
    @location(5) sigma: f32,
    @location(6) max_alpha: f32,
    @location(7) mode: f32,
    @location(8) border_width: f32,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VSOut {
    var out: VSOut;
    let inst = instances[instance_index];
    let pos4 = vec4<f32>(position, 0.0, 1.0);
    let world = transformUniform.transform * inst.model * pos4;
    out.position = world;
    out.local_pos_ndc = position;
    out.color = inst.color;
    out.rect_size_px = inst.rect_size_px;
    out.corner_radius_px = inst.corner_radius_px;
    out.glow_radius_px = inst.glow_radius_px;
    out.sigma = inst.sigma;
    out.max_alpha = inst.max_alpha;
    out.mode = inst.mode;
    out.border_width = inst.border_width;
    return out;
}

fn roundedRectSDF(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - (half_size - vec2<f32>(r, r));
    let outside = max(q, vec2<f32>(0.0, 0.0));
    return length(outside) - r;
}

@fragment
fn fs_main(
    @location(0) local_pos_ndc: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) rect_size_px: vec2<f32>,
    @location(3) corner_radius_px: f32,
    @location(4) glow_radius_px: f32,
    @location(5) sigma: f32,
    @location(6) max_alpha: f32,
    @location(7) mode: f32,
    @location(8) border_width: f32,
) -> @location(0) vec4<f32> {
    // The quad covers the inner rect + glow_radius on each side.
    // Total quad size in px = rect_size_px + 2 * glow_radius_px
    // local_pos_ndc is in [-1, 1], mapping to the full oversized quad.
    let half_total_size = 0.5 * (rect_size_px + vec2<f32>(2.0 * glow_radius_px, 2.0 * glow_radius_px));
    let p_px = local_pos_ndc * half_total_size;

    // Inner rect half-size
    let half_inner_size = 0.5 * rect_size_px;
    let r = max(corner_radius_px, 0.0);

    // SDF distance to the inner rounded rect
    let dist = roundedRectSDF(p_px, half_inner_size, r);

    // Clamp sigma to avoid division by zero
    let s = max(sigma, 0.001);

    // Soft glow mode: Gaussian falloff outside the shape
    let outer_dist = max(dist, 0.0);
    let gaussian_glow = exp(-(outer_dist * outer_dist) / (2.0 * s * s));

    // Border mode: narrow Gaussian band centered on the edge (dist ≈ 0)
    let border_sigma = max(border_width * 0.5, 0.5);
    let gaussian_border = exp(-(dist * dist) / (2.0 * border_sigma * border_sigma));

    // Mode 2.0: SDF perimeter glow (Neon/Perimeter style)
    // thickness = border_width
    // glow_radius = sigma
    let thickness = border_width;
    let core = smoothstep(thickness * 0.5 + 1.0, thickness * 0.5 - 1.0, abs(dist));
    let exponential_glow = exp(-abs(dist) / max(sigma, 0.001));
    let perimeter_glow = max(core, exponential_glow);

    // Mix based on mode field
    // 0.0 = Gaussian Glow
    // 1.0 = Gaussian Border
    // 2.0 = SDF Perimeter Glow
    var raw_alpha: f32;
    if (mode > 1.5) {
        raw_alpha = perimeter_glow * color.a;
    } else {
        raw_alpha = mix(gaussian_glow, gaussian_border, clamp(mode, 0.0, 1.0)) * color.a;
    }
    
    let final_alpha = min(raw_alpha, max_alpha);

    // Premultiplied alpha output
    return vec4<f32>(color.rgb * final_alpha, final_alpha);
}
