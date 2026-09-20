struct PortalState {
    active_seats: f32,
    health: f32,
    phase: f32,
    stage_tint: f32,
}

@group(0) @binding(0)
var<uniform> state: PortalState;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var output: VertexOut;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return output;
}

fn sd_circle(point: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    return length(point - center) - radius;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(320.0, 180.0);
    let uv = position.xy / dimensions;
    let aspect_point = vec2<f32>((uv.x - 0.5) * (dimensions.x / dimensions.y), uv.y - 0.5);

    let paper = vec3<f32>(0.018, 0.024, 0.031);
    let grid_x = smoothstep(0.985, 1.0, cos(uv.x * 80.0));
    let grid_y = smoothstep(0.985, 1.0, cos(uv.y * 44.0));
    var color = paper + vec3<f32>(0.008, 0.014, 0.018) * (grid_x + grid_y);

    let health_color = mix(
        vec3<f32>(0.20, 0.66, 0.56),
        vec3<f32>(0.88, 0.47, 0.20),
        clamp(state.health, 0.0, 1.0),
    );
    let core_distance = sd_circle(aspect_point, vec2<f32>(0.0, -0.02), 0.105);
    let halo = exp(-max(core_distance, 0.0) * 18.0);
    color += health_color * halo * (0.14 + 0.08 * state.stage_tint);
    color = mix(color, health_color * 0.82, smoothstep(0.012, -0.012, core_distance));

    let active_count = min(state.active_seats, 16.0);
    for (var seat: u32 = 0u; seat < 16u; seat = seat + 1u) {
        if (f32(seat) < active_count) {
            let column = f32(seat % 8u);
            let row = f32(seat / 8u);
            let center = vec2<f32>(
                (column - 3.5) * 0.145,
                0.285 + row * 0.12,
            );
            let pulse = 0.004 * sin(state.phase + f32(seat) * 0.71);
            let distance = sd_circle(aspect_point, center, 0.032 + pulse);
            let seat_ink = mix(
                vec3<f32>(0.26, 0.56, 0.76),
                vec3<f32>(0.65, 0.48, 0.80),
                fract(state.stage_tint + f32(seat) * 0.13),
            );
            color += seat_ink * exp(-max(distance, 0.0) * 70.0) * 0.18;
            color = mix(color, seat_ink, smoothstep(0.006, -0.006, distance));
        }
    }

    let vignette = 1.0 - smoothstep(0.40, 0.82, length(aspect_point));
    color *= 0.70 + 0.30 * vignette;
    return vec4<f32>(color, 1.0);
}
