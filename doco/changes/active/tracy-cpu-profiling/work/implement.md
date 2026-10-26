# Implementation design

## 1. Baseline and goals
基线 `481b33a` 已提交用户工作区白炉/IBL 改动。当前架构见 ../../../../architecture.md；资产线程边界见 ../../../../specs/asset-runtime.md。
入口 src/main.rs 的 -t 安装旧 wgpu-subscriber；src/lib.rs 无条件添加 LogPlugin。core/window.rs 的 resumed 和 RedrawRequested 分别调用 app.update。render/stage/mod.rs 将实际渲染放入 commands.queue 并逐 Stage submit。egui 的 surface acquire/submit/present 在 src/editor/gui/mod.rs，细分 UI 工作在 src/egui_renderer.rs。scene/loading.rs 有命名的 CPU worker，controller.rs 执行提交。当前未启用 Bevy trace/multi_threaded，wgpu/egui 共用 profiling 1.0.18。

## 2. Overall approach
根 feature profile-tracy 传递至 core/render 并开启 bevy_app/trace、bevy_ecs/trace、bevy_log/tracing-tracy。core 提供编译开关控制的 scope 宏、frame mark、线程命名及由 App Resource 持有的客户端。WgpuPbrPlugin 最先初始化客户端，再由现有 LogPlugin 安装唯一 subscriber，之后才初始化渲染与后台线程。详细档仅增加 profiling/profile-with-tracy，不再安装 subscriber。
固定 Tracy client 0.18.4 / sys 0.28.0 对应 GUI 0.13.1；通过 Cargo.lock 保持一致。启用 ondemand 和 only-localhost。业务 scopes 使用 info 级 tracing；Tracy 模式显式添加 wgpu_pbr=info，避免 Bevy 默认 wgpu=error 前缀连同根 crate 一起过滤。fmt layer 关闭 ANSI，防止 tracing-tracy 复用格式化字段时污染 zone 名。RUST_LOG 仍可覆盖过滤，操作文档说明。不开全局 trace 噪声，业务宏 feature 关闭时不求值字段。

## 3. APIs and data model
core 新增 profiling 模块：init(&mut App)、name_thread(&str)、frame_mark()，以及 profile_scope! 静态名称/字段宏。仅用于观测，不进入场景持久化。客户端资源从插件构建持有到 App 退出；没有客户端时帧/命名辅助函数安全无操作，便于未安装插件的库调用。详细档依赖埋点要求先启动客户端；自建 wgpu App 的调用者需调用 init，文档说明。
window.rs 将两处更新统一到私有 update_frame：FrameCPU zone 仅包 app.update，退出 zone 后打一次 mark。初始化、退出/失败呈现不会造成重复帧标记。线程只在启动时命名。
Tracy 构建在 main 中解析 -t 后、启动 App 前报告互斥错误并退出 2；非 Tracy 路径保留兼容性，避免重复添加 LogPlugin 的全局初始化（旧 subscriber 路径由根插件识别已有默认 dispatcher 后跳过 LogPlugin）。

## 4. Algorithms and rules
Stage scope 必须放入 commands.queue 的执行闭包，不跨线程携带 entered guard。每 Stage 包实际 system 执行和提交，finish 与 submit 使用独立词法作用域，不能将二者混测。直接窗口 blit 和编辑器呈现均标记 acquire/present。egui 的 end_pass/platform/tessellation/texture upload/buffer update/encode 分层观测。worker 仅在实际 job.load 周围进入 scope，不把 Condvar 空闲等待当作加载时间；主线程 poll/commit 分别测量。
不更改数据流、同步、渲染顺序、窗口呈现模式和资产预算，不新增 GPU 等待。初始帧视为启动样本，稳定态分析需排除。所有 zones 是 CPU wall time，可能包含驱动/调度等待。

## 5. Fixed decisions and discretion
固定：两档编译特性、现有 LogPlugin 为 Tracy subscriber 唯一安装者、CPU 优先、默认关闭、按应用更新标帧。允许局部 helper 提取和 scope 命名调整。为控制开销不逐实体埋点、不启用内存分配跟踪、不额外采集每 zone 调用栈。无阻塞设计问题。

## 6. Verification and documentation impact
检查默认/基础/详细 feature 依赖树与 cargo check/build；运行 cargo test --workspace --lib --tests；验证 -t 互斥错误。可用时真实运行并连接 Tracy/capture，确认帧标记、Stage 和 Acquire/Submit/Present 标签；工具缺失应记录未验证项。对比相同优化构建开关的性能属于采集后人工分析，不预设瓶颈。
新增当前 specs/profiling.md 并在 architecture.md 记录观测入口、开关与兼容性。执行结束保持 active，不自动 complete/archive，不自动提交本次实现。
