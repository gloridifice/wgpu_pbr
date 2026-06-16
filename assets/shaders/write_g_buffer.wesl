import package::libs::vertex::{ VertexInput };
import package::libs::primary::pbr_type::{ StandardMaterial, PBRSurface };
import package::libs::primary::pbr_type;
import package::libs::primary::global_bindings::{
    camera, light,
};
import package::libs::primary::material_bindings::{
    pbr_mat, sample_albedo, sample_metallic_roughness_ao, sample_normal
};
import package::libs::primary::object_bindings::{
    transform,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) tex_coord: vec2<f32>,
    @location(4) world_pos: vec3<f32>,
};

struct FragmentOutput {
    @location(0) g_buffer: vec4<u32>,
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    let model_mat = transform.model;

    var out: VertexOutput;
    out.vertex_color = model.color;
    out.world_pos = (model_mat * vec4<f32>(model.position, 1.0)).xyz;
    out.normal = transform.normal * model.normal;
    out.tangent = transform.normal * model.tangent;
    out.tex_coord = model.tex_coord;
    out.clip_position = camera.view_proj * vec4<f32>(out.world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let uv = in.tex_coord;
    let base_color = sample_albedo(uv);

    let n_normal = normalize(in.normal);
    let n_tangent = normalize(in.tangent);
    let bitangent = cross(n_normal, n_tangent);
    let tbn = mat3x3<f32>(n_tangent, bitangent, n_normal);
    let tangent_space_normal = sample_normal(uv) * 2.0 - 1.0;
    let normal = normalize(tbn * tangent_space_normal);

    var surface: PBRSurface = pbr_type::pbr_surface_new();
    var material: StandardMaterial = pbr_type::standard_material_new();
    let sampled = sample_metallic_roughness_ao(uv);
    surface.normal = normal;
    material.base_color = base_color;
    material.metallic = sampled.metallic;
    material.perceptual_roughness = sampled.roughness;
    material.reflectance = pbr_mat.reflectance;
    surface.material = material;

    var o: FragmentOutput;
    o.g_buffer = pbr_type::pack_g_buffer(surface);

    return o;
}
