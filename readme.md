# Wgpu PBR
A realtime PBR renderer project for personal learning. Now it's in progress.
![cover](readme/cover.gif)

Powered by

- [wgpu](https://wgpu.rs/): A cross-platform, safe, pure-rust graphics API.
- [winit](https://github.com/rust-windowing/winit): Cross-platform window creation and management in Rust.
- [cgmath](https://github.com/rustgd/cgmath): Mathmatic library.
- [bevy_ecs](https://docs.rs/bevy_ecs/latest/bevy_ecs/): Entity Component System architecture in Rust by bevy engine.

## Roadmap
- [x] Transform & Camera & Phong pipeline
- [x] Directional light shadow mapping
- [x] Normal mapping
- [x] Deferred rendering pipeline
- [x] Microfact directional lighting & point lighting
- [ ] Color management
- [ ] Microfact image based lighting
  - [x] Environment map prefiltering (GGX distribution)
  - [ ] Diffuse irradiance spherical harmonics pre-calculation
  - [ ] HDRI to cubemap converting
- [ ] Clear coat model
- [ ] Transparent pipeline
- [ ] Better user interface
- [ ] Cascade shadow mapping

## Screenshot

![metallic](readme/metallic.png)
![shadow mapping](readme/shadow_mapping.png)
