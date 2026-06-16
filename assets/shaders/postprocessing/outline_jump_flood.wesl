import package::libs::vertex::{ FullscreenV2F };

struct JumpFloodUniform {
    step_width: f32,
};

@group(0) @binding(0) var source_tex: texture_2d<f32>;
@group(0) @binding(1) var<uniform> jump_flood: JumpFloodUniform;

fn better_candidate(pixel: vec2<i32>, current: vec2<f32>, candidate: vec2<f32>) -> vec2<f32> {
    if candidate.x < 0.0 {
        return current;
    }
    if current.x < 0.0 {
        return candidate;
    }

    let fp = vec2<f32>(pixel) + vec2<f32>(0.5);
    let current_dist = distance(fp, current);
    let candidate_dist = distance(fp, candidate);
    if candidate_dist < current_dist {
        return candidate;
    }
    return current;
}

@fragment
fn fs_main(in: FullscreenV2F) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(source_tex, 0));
    let pixel = vec2<i32>(in.clip_position.xy);
    let step_width = i32(jump_flood.step_width);

    var best = textureLoad(source_tex, pixel, 0).xy;

    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let sample_pixel = clamp(pixel + vec2<i32>(x, y) * step_width, vec2<i32>(0), dims - vec2<i32>(1));
            let candidate = textureLoad(source_tex, sample_pixel, 0).xy;
            best = better_candidate(pixel, best, candidate);
        }
    }

    return vec4<f32>(best, 0.0, 1.0);
}
