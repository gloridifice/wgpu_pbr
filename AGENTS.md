# AGENTS.md

## Documentation and Code Conflicts

When documentation conflicts with the existing code, follow the code's implementation and conventions, and list all conflicts found in the final response.

## Build and Run

```powershell
cargo run                     # Start the real-time PBR renderer (requires a GPU)
cargo build
cargo test --workspace --lib --tests # Default unit/integration tests; no GPU required
cargo test -p lentille_render --lib # Run only the render crate's default tests
cargo run -- --scene scenes/demo.scn.ron # Open a specific scene (path relative to the assets root)
cargo run --example scene_smoke # Validate the scene lifecycle with a real window and GPU
```

There is no CI, lint/format/clippy configuration, or pre-commit hook.

## Architecture

- **`wgpu_pbr` (root crate)**: Binary and library. The entry point, `src/main.rs`, assembles `WgpuPbrPlugin` through bevy_app.
- **`lentille_core`**: Window (`winit`), input, and time abstractions.
- **`lentille_math`**: Re-exports `cgmath` and provides a custom `Color` type.
- **`lentille_render`**: The engine's core: render graph, pipelines, shaders, materials, lighting, skyboxes, shadow mapping, transparency passes, and deferred rendering.
- **`lentille_wgpu_utils`**: Thin wgpu utilities (bind group layouts, texture descriptors, sampler descriptors, and fullscreen pipeline glue code).

The project uses **bevy_ecs + bevy_app**, not the full Bevy engine. Rendering is driven by custom scheduling labels (`RenderPreparedStartup`, `FrameSets`, `PreStage`, `OpaqueStage`, and `TransparentStage`).

## Version Notes

The root crate and all five workspace crates use `edition = "2024"`. Current dependencies include Bevy 0.18 and wgpu 29; the compiler toolchain must satisfy the MSRV requirements of both.

## Shader System (WESL)

Shaders reside in `assets/shaders/`. `ShaderLoader` uses WESL 0.4 to compile `package::...` modules and returns `ShaderSource::Wgsl`. The previous naga_oil startup scanning/composition workflow is no longer used.

Compiled WGSL and dependency timestamps are cached in `.cache/shaders/`. Set `WGPU_PBR_NO_SHADER_CACHE` to disable caching. Follow the current WESL package paths when adding shaders or imports. The asset helper retains the name `AssetPath::new_shader_wgsl`, but actually appends the `.wesl` extension.

## Scenes and Assets

By default, the application loads `assets/scenes/minimal.scn.ron`, which uses only a procedural plane and the application's default environment. `demo.scn.ron` references models/textures in the repository; any additional external assets must be supplied separately.

Serialized components store parameters and asset references relative to the assets root; runtime components store GPU indices, buffers, bindings, and derived results. CPU model import is separate from GPU upload. Meshes/textures are cached per scene scope, and each entity has its own transform buffer.

Scenes use a versioned RON wrapper around Bevy 0.18 DynamicScene, not BSN. The implementation is in `src/scene/`, and specifications are in `doco/specs/`. The Content Browser below the Viewport uses a UE-style directory tree and card grid, displaying only folders and `.scn.ron` files. It supports searching the current directory, refreshing, and adjusting card size. Double-clicking opens a scene through the scene controller; no actual thumbnails are generated.

## Tests

Default pure-Rust tests are located in `lentille_render`, the root `tests/` directory, and the browser panel module. GPU cache tests are ignored by default and can be run explicitly with `cargo test -p lentille_render --lib gpu_assets -- --ignored --test-threads=1`.

Currently, `cargo test --workspace` also runs legacy macro documentation examples. The three incomplete examples in `lentille_wgpu_utils/src/bind_group_macro.rs` are not compilable doctests; passing the default unit/integration tests does not mean those three examples pass.

<!-- DOCO:START -->
<!-- doco:entry template=v2 -->
## Doco

For current project documentation and explicitly tracked changes, use the
`doco` skill. Read `.agents/skills/doco/SKILL.md` before creating, executing, completing,
or archiving a change. Perform only the requested phase.
A doco change package is optional for implementation and is not an
implementation-history mechanism: archiving retains only proposal.md. Routine
behavior fixes and implementation-detail changes may proceed without creating
a doco change unless the user explicitly requests tracking. Use Git, pull
requests, or release notes for implementation history. Regardless of tracking,
update current architecture and specs when their documented facts or contracts
change.
Use the doco library nearest to the working directory, or select one with
`doco --root <library-root> ...`. Child libraries keep their own documents and
changes but reuse this installed skill; do not generate child skill or Agent
entry files. Resolve the skill path above from this entry's directory and
`doco/` paths from the selected library. Parent documents describe shared
constraints; child documents describe local facts without automatic overrides.
`doco list` includes all project libraries, current first; write commands and
change IDs remain local to the selected library.
Start from its `doco/architecture.md` and relevant current specs and decisions.
When implementing a selected tracked change, use its proposal and any present
design and task files. Treat completed changes, archived changes, and
`doco/tmp/` as non-current material; consult them only when explicitly needed.
<!-- DOCO:END -->
