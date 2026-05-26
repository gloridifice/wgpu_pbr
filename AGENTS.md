# AGENTS.md

## 构建与运行

```powershell
cargo run                     # 启动实时 PBR 渲染器（需要 GPU）
cargo build
cargo test                    # 纯单元测试，不需要 GPU
cargo test -p lentille_render # 仅运行渲染 crate 的测试
```

没有 CI、lint/format/clippy 配置，也没有 pre-commit 钩子。

## 架构

- **`wgpu_pbr`（根 crate）**：二进制 + 库。入口 `src/main.rs` 通过 bevy_app 组装 `WgpuPbrPlugin`。
- **`lentille_core`**：窗口（`winit`）、输入和时间抽象。
- **`lentille_math`**：重新导出 `cgmath` + 自定义 `Color` 类型。
- **`lentille_render`**：引擎主体 — 渲染图、管线、着色器、材质、光照、天空盒、阴影映射、透明通道、延迟渲染。
- **`lentille_wgpu_utils`**：薄层 wgpu 辅助工具（绑定组布局、纹理描述符、采样器描述符、全屏管线胶水代码）。

项目使用 **bevy_ecs + bevy_app**（非完整 Bevy 引擎）。渲染执行通过自定义调度标签驱动（`RenderPreparedStartup`、`FrameSets`、`PreStage`、`OpaqueStage`、`TransparentStage`）。

## 版本说明

根 `Cargo.toml` 使用 `edition = "2021"`。四个工作区 crate 全部使用 `edition = "2024"`。两者需要一起编译，因此 Rust 工具链必须支持 2024 版本（Rust >= 1.85）。

## 着色器系统（naga_oil）

着色器位于 `assets/shaders/` 下。管线使用 `naga_oil::compose::Composer`：

1. 启动时，`ShaderLoader::from_world` 先扫描 `shaders/libs/primary/*.wgsl`，再扫描 `shaders/libs/*.wgsl`，将两者都加载为可组合模块。
2. 通过 `ShaderLoader::load_source()` 加载着色器时，`make_naga_module` 使用预加载的可组合模块解析 `#import` 指令。
3. 结果为 `ShaderSource::Naga`（预编译的 Naga IR 模块），而非原始 WGSL。

**添加/修改含导入的着色器时**：确保被导入的模块在 `shaders/libs/`（或 `shaders/libs/primary/`）中，以便组合器在启动时能发现它们。

## 资源文件（不在仓库中）

`assets/models/` 和 `assets/textures/hdr/` 已被 gitignore。应用会从这些目录引用 `.gltf`/`.glb` 模型和 `.hdr` 环境贴图，运行前需手动放入。

## 测试

所有测试都在 `lentille_render` 中，为纯 Rust 单元测试（无需 GPU/窗口）。运行方式：`cargo test -p lentille_render`。
