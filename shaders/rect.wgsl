struct TransformUniform {
    transform: mat4x4<f32>,
};

@group(1) @binding(0)
var<uniform> transformUniform: TransformUniform;

struct RectInstanceData {
    model: mat4x4<f32>,
    color: vec4<f32>,
    corner_radius_px: f32,
    border_thickness_px: f32,
    _pad0: vec2<f32>,
    border_color: vec4<f32>,
    rect_size_px: vec2<f32>,
    _pad1: vec2<f32>,
    _pad2: vec4<f32>,
};

@group(3) @binding(0)
var<storage, read> instances: array<RectInstanceData>;

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos_ndc: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) corner_radius_px: f32,
    @location(3) border_thickness_px: f32,
    @location(4) border_color: vec4<f32>,
    @location(5) rect_size_px: vec2<f32>,
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
    // Pass along local coordinates in the range [-1, 1]
    out.local_pos_ndc = position;
    out.color = inst.color;
    out.corner_radius_px = inst.corner_radius_px;
    out.border_thickness_px = inst.border_thickness_px;
    out.border_color = inst.border_color;
    out.rect_size_px = inst.rect_size_px;
    return out;
}

@fragment
fn fs_main(
    @location(0) local_pos_ndc: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) corner_radius_px: f32,
    @location(3) border_thickness_px: f32,
    @location(4) border_color: vec4<f32>,
    @location(5) rect_size_px: vec2<f32>,
) -> @location(0) vec4<f32> {
    // Convert local position in [-1,1] to pixel space centered at origin
    let half_size_px = 0.5 * rect_size_px;
    let p_px = local_pos_ndc * half_size_px; // now in pixels relative to center

    // Compute SDF for rounded rectangle (outer)
    let r = max(corner_radius_px, 0.0);
    let q = abs(p_px) - (half_size_px - vec2<f32>(r, r));
    let outside = max(q, vec2<f32>(0.0, 0.0));
    let dist_outer = length(outside) - r;

    // Anti-alias factor
    let aa = 1.0; // 1 px AA
    let alpha_fill = clamp(0.5 - dist_outer / aa, 0.0, 1.0);

    // Border handling
    var out_color: vec4<f32> = color;
    if (border_thickness_px > 0.0) {
        let inner_r = max(r - border_thickness_px, 0.0);
        let q_in = abs(p_px) - (half_size_px - vec2<f32>(inner_r, inner_r));
        let outside_in = max(q_in, vec2<f32>(0.0, 0.0));
        let dist_inner = length(outside_in) - inner_r;

        // border region where outer <= 0 and inner > 0 (approximately)
        let border_mask = clamp(0.5 - dist_outer / aa, 0.0, 1.0) * (1.0 - clamp(0.5 - dist_inner / aa, 0.0, 1.0));
        // mix border and fill based on which region we're in
        let fill_mask = clamp(0.5 - dist_inner / aa, 0.0, 1.0);
        let border_col = vec4<f32>(border_color.rgb, border_color.a * border_mask);
        let fill_col = vec4<f32>(color.rgb, color.a * fill_mask);
        // alpha composite border over fill
        out_color = border_col + fill_col * (1.0 - border_col.a);
        // Ensure we also apply outer edge AA
        out_color.a = max(out_color.a, alpha_fill);
    } else {
        out_color.a = alpha_fill;
    }

    return out_color;
}


