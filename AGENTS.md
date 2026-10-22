# AGENTS.md

## 构建与运行

```powershell
cargo run                     # 启动实时 PBR 渲染器（需要 GPU）
cargo build
cargo test --workspace --lib --tests # 默认单元/集成测试，不需要 GPU
cargo test -p lentille_render --lib # 仅运行渲染 crate 的默认测试
cargo run -- --scene scenes/demo.scn.ron # 打开指定场景（assets 根相对路径）
cargo run --example scene_smoke # 真实窗口/GPU 场景生命周期验证
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

根 crate 和五个工作区 crate 全部使用 `edition = "2024"`。当前依赖 Bevy 0.18 与 wgpu 29；编译工具链需要同时满足这些依赖的 MSRV。

## 着色器系统（WESL）

着色器位于 `assets/shaders/`。`ShaderLoader` 使用 WESL 0.4 编译 `package::...` 模块，返回 `ShaderSource::Wgsl`，不再使用 naga_oil 的启动扫描/组合流程。

编译后的 WGSL 和依赖时间戳缓存在 `.cache/shaders/`。可设置 `WGPU_PBR_NO_SHADER_CACHE` 禁用缓存。添加着色器或导入时遵循当前 WESL 包路径；资源辅助函数 `AssetPath::new_shader_wgsl` 名称保留，但实际追加 `.wesl` 后缀。

## 场景与资源

默认加载 `assets/scenes/minimal.scn.ron`，仅使用程序化平面和应用默认环境。`demo.scn.ron` 引用仓库中的模型/纹理；额外外部资产仍需自行提供。

序列化组件保存参数和 assets 根相对资源引用；运行时组件保存 GPU 索引、Buffer、绑定与派生结果。模型 CPU 导入与 GPU 上传分离，网格/纹理按场景作用域缓存，每实体变换 Buffer 独立。

场景使用 Bevy 0.18 DynamicScene 的版本化 RON 封装，不是 BSN。相关实现位于 `src/scene/`，规范见 `doco/specs/`。Viewport 下方 Content Browser 使用 UE 式目录树与卡片网格，只浏览文件夹和 `.scn.ron`；支持当前目录搜索、刷新和卡片大小调整，双击通过场景控制器打开，不生成真实缩略图。

## 测试

默认纯 Rust 测试位于 `lentille_render`、根 `tests/` 和浏览面板模块。GPU 缓存测试默认忽略，可显式运行：`cargo test -p lentille_render --lib gpu_assets -- --ignored --test-threads=1`。

当前 `cargo test --workspace` 还会运行旧宏文档示例；`lentille_wgpu_utils/src/bind_group_macro.rs` 的三个未补全示例不能作为可编译 doctest，通过默认单元/集成测试不代表这三个示例已通过。

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
