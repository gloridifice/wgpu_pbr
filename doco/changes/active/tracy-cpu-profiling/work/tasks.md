# Execution tasks

- [x] 1.1 接入 feature、客户端生命周期与单一 subscriber
  - Design: [implementation](implement.md)
  - Acceptance: 默认不链接 Tracy；基础与详细档依赖一致；旧 -t 与 Tracy 有明确互斥错误。
- [x] 1.2 补齐主循环、渲染、呈现与场景加载 CPU 埋点
  - Dependencies: 1.1
  - Acceptance: 每 update 一次帧标记；实际延迟闭包、finish/submit、acquire/present 和后台加载可独立定位；不改变渲染算法。
- [x] 2.1 更新当前架构和 profiling 操作契约
  - Dependencies: 1.1, 1.2
  - Acceptance: 命令、版本匹配、生命周期、采集范围、开销和限制与实现一致。
- [x] 2.2 构建、回归与实际采集验证
  - Dependencies: 1.1, 1.2, 2.1
  - Acceptance: 默认/基础/详细构建及默认测试通过；实际采集确认帧与 scopes，未执行项保留未完成记录。

## Verification
- `cargo check --locked`、`cargo check --locked --features profile-tracy`、`cargo check --features profile-tracy-deps` 通过。
- `cargo tree --locked -i tracy-client` 默认构建报告无此包（预期）；详细档依赖树统一至 tracy-client 0.18.4 / sys 0.28.0。
- `cargo build --locked --profile profiling --features profile-tracy-deps --bin wgpu_pbr` 通过；基础档同 profile 的 bin 和 scene_smoke example 构建通过。
- 基础与详细档二进制 `-t` 均返回 2，输出明确的互斥提示，未创建窗口。
- 实际启动优化版应用，tracy-capture 连接 localhost；详细档采集 10 秒，3285 帧、1,684,439 zones；基础档采集 5 秒，1222 帧、187,834 zones。两次应用均由自身窗口关闭事件正常退出 0。
- `tracy-csvexport` 导出并检查 FrameCPU、RenderStage、RenderCamera、EncoderFinish、QueueSubmit、SurfaceAcquire、SurfacePresent、EguiTessellate、SceneCpuLoad、SceneCommit；详细档另确认纹理/buffer 上传与依赖内部 scopes，基础档无 wgpu-core 内部 zones，全部名称无 ANSI。capture 起止跨越不完整帧，完整 FrameCPU 数分别为 3282 / 1219，非重复标记。
- 首轮真实采集发现 wgpu=error 屏蔽 wgpu_pbr，修正明确 target 过滤并重新采集后上述检查通过；保留最终成功样本 [cpu.tracy](../../../../../target/tracy-validation/cpu.tracy)、[basic.tracy](../../../../../target/tracy-validation/basic.tracy) 和对应 CSV/log（忽略文件，不提交）。
- 已构建的 scene_smoke.exe（profile-tracy 构建）通过，输出 SCENE_SMOKE_OK，覆盖 817 个异步加载帧、冻结编辑、resize、取消和回滚。
- 最终 `cargo test --locked --workspace --lib --tests`：26 passed、2 ignored（显式 GPU 单测），无失败。未运行 legacy doctests。没有新增测试专用生产接口；实际 capture 与现有生命周期 smoke 是主要验证边界。
- 修改过的 Rust 文件定向 rustfmt --check 和 git diff --check 通过；已有 dead_code/unused import/lifetime 编译警告未扩大处理。
- 当前架构与 profiling spec 已同步；doco check 机械检查通过。保持 active，不执行 complete/archive。此采集只证明接入有效，未完成稳定态 A/B 开销测量或瓶颈诊断。

Blocked: none
