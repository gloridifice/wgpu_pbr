struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>
}

struct CameraUniform {
    view_proj: mat4x4<f32>
}

struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
    lights_nums: vec4<u32>,
}

struct PointLight {
    color: vec4<f32>,
    position: vec4<f32>,
    intensity: f32,
    distance: f32,
    decay: f32,
}

@group(0) @binding(0) var g_samp: sampler;
@group(0) @binding(1) var world_pos_tex: texture_2d<f32>;
@group(0) @binding(2) var normal_tex: texture_2d<f32>;
@group(0) @binding(3) var color_tex: texture_2d<f32>;
@group(0) @binding(4) var tex_coord_tex: texture_2d<f32>;

@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var<uniform> light: LightUniform;

@group(2) @binding(0) var<storage, read> point_lights: array<PointLight>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = textureSample(world_pos_tex, g_samp, in.uv).xyz;
    let normal = textureSample(normal_tex, g_samp, in.uv).xyz;
    let color = textureSample(color_tex, g_samp, in.uv);

    var surface_color = vec3<f32>(0);
    surface_color += max(dot(-light.direction, normal), 0.0) * light.color.rgb * light.intensity;

    let point_lights_num = light.lights_nums.x;

    for(var i = 0u; i < point_lights_num; i += 1u) {
        let li = point_lights[i];
        let world2light = li.position.xyz - world_pos;
        let dist = length(world2light);
        let dir = normalize(world2light);

        let radiance = li.intensity / (li.decay * pow(dist, 2.0));
        let nDotL = max(-dot(normal, dir), 0.0);

        surface_color += radiance * nDotL * li.color.xyz;
        // surface_color += vec3<f32>(1.0);
    }

    // return vec4<f32>(color.xyz * lightFactor.xyz, 1.0);
    // return vec4<f32>(surface_color * color.rgb, color.a);
    return vec4<f32>(normal, color.a);
}
