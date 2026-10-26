# Execution tasks

- [x] 1.1 实现场景 HDR ping-pong、SDR 输出及跨阶段状态传递
  - Design: [implementation](implement.md)
  - Acceptance: A/B 为 Rgba16Float，按处理步骤交换；透明读取本帧背景；逐相机隔离；resize 和输出目标保持正确。

- [x] 1.2 接入 ToneMapping 和显示叠加边界
  - Dependencies: 1.1
  - Acceptance: 默认 0 EV 的 Reinhard 输出到 sRGB；描边/Gizmo 在其后；移除 IBL 高亮截断；Viewport 和窗口输出不复制不兼容格式。

- [x] 1.3 同步示例和场景加载资源预算
  - Dependencies: 1.1, 1.2
  - Acceptance: 示例消费 SDR 结果；活动和候选相机预算反映新增 HDR 资源；场景格式及加载事务不变。

- [x] 2.1 验证颜色数值、阶段交换与生命周期
  - Dependencies: 1.1, 1.2, 1.3
  - Acceptance: 新 shader 编译验证、真实 GPU 像素和连续帧验证、resize/scene_smoke 有实际结果；未执行项明确注明。

- [x] 2.2 更新当前文档并执行最终检查
  - Dependencies: 2.1
  - Acceptance: architecture/渲染颜色契约与代码一致；构建、默认测试、示例检查、git diff --check、doco check 均记录；保留 active 状态。

## Verification

- 已检查基线：git status --short 仅 AGENTS.md 已有修改；doco list 中另外两个 active 包为已实现的场景加载/浏览基线，本包不改其生命周期。
- `cargo check --workspace --all-targets`：通过；随后 `cargo build`、`cargo check --examples` 最终复验通过。
- `cargo test --workspace --lib --tests`：最终通过，23 passed、2 ignored（GPU），不包含 legacy doctest。
- `cargo test -p lentille_render --lib tonemapping -- --include-ignored --test-threads=1`：通过，2 passed。真实阶段链路验证 1/4/16 高亮、0.18 灰阶、曝光、部分覆盖、显示空间叠加、双相机及 resize；真实 blit 验证 BGRA/RGBA sRGB 和 RGBA UNORM 输出字节。
- 负对照：临时移除透明准备的背景复制、保留 swap，`gpu_hdr_colors` 在未覆盖像素 (4,0) 以实际 0 / 期望 231 失败；恢复后通过。该验证证明背景保留契约，不宣称是旧代码 API 下运行的新测试。
- `cargo test -p lentille_render --lib -- --ignored --test-threads=1`：最终通过，资产 GPU 测试和 HDR GPU 测试共 2 passed。
- `cargo run --example scene_smoke`：最终输出 SCENE_SMOKE_OK，2389 个异步加载帧，覆盖冻结编辑、加载中 resize、取消、后续加载、失败回滚、save/reload 和 CSM。环境有 Epic Vulkan overlay JSON 缺失及 egui framebuffer 偏好告警，未阻止验证。
- 已仅对修改的 Rust 文件运行 rustfmt（skip_children=true）。`git diff --check` 通过，无空白错误；Git 提示部分文件未来会进行 LF/CRLF 转换。
- 当前 architecture、asset-runtime 和新增 render-color 契约已同步。旧 RenderContext/透明对象注释已修正；DeferredComputePipeline 旧名称仍对应 raster 管线，文档明确说明。
- 未做人工视觉质量评审、HDR 显示器测试或性能基准；它们不是本包的数值/生命周期验收替代品。没有执行 complete/archive，也没有提交 Git。
- `doco check hdr-scene-color`：最终通过；`doco list` 显示 active、5/5，其他 active 包未变更。
- 代码规模（不含用户 AGENTS.md 和文档）：生产 Rust/shader +453/-205 行；新增测试 +346 行；窗口示例 +8/-54 行。测试未引入生产专用开关或导出。
