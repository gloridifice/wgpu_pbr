import package::libs::vertex::{ FullscreenV2F };

struct OutlineUniform {
    color: vec4<f32>,
    thickness: f32,
};

@group(0) @binding(0) var nearest_tex: texture_2d<f32>;
@group(0) @binding(1) var mask_tex: texture_2d<f32>;
@group(0) @binding(2) var<uniform> outline: OutlineUniform;

@fragment
fn fs_main(in: FullscreenV2F) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.clip_position.xy);
    let mask = textureLoad(mask_tex, pixel, 0).xy;
    if mask.x >= 0.0 {
        discard;
    }

    let nearest = textureLoad(nearest_tex, pixel, 0).xy;
    if nearest.x < 0.0 {
        discard;
    }

    let current = vec2<f32>(pixel) + vec2<f32>(0.5);
    let nearest_pixel = vec2<i32>(nearest);
    let nearest_center = vec2<f32>(nearest_pixel) + vec2<f32>(0.5);
    if all(nearest_pixel == pixel) {
        discard;
    }

    let dist = distance(current, nearest_center);
    if dist <= 0.5 || dist > outline.thickness {
        discard;
    }

    return outline.color;
}
