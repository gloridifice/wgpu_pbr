<!-- doco:lifecycle v=1 created-at=2026-10-26T08:20:33Z completed-at=- archived-at=- -->
# Tracy CPU 抓帧

## Purpose
GPU 抓帧暂未解释低帧率，需要可关联应用帧、ECS system、实际渲染阶段和呈现等待的 CPU 时间线，先测量再优化。

## Scope and acceptance
- 新增默认关闭的 `profile-tracy`；复用 Bevy tracing 与单一 LogPlugin subscriber，提供应用帧标记、线程名称和关键 CPU scopes。
- `profile-tracy-deps` 在基础档上开启 wgpu/egui 已有 profiling 埋点；新增继承 release 并保留符号的 profiling 构建配置。
- 每次主循环应用更新恰好一个帧标记，包括 resumed 的初次更新；标记不依赖相机数、Stage 数或是否成功呈现。
- 时间线覆盖实际延迟渲染闭包、Stage、encoder finish、queue submit、surface acquire/present、egui tessellation/upload 和场景后台加载/提交。
- 保留旧 `-t` 的非 Tracy 行为；Tracy 构建拒绝与 `-t` 并用并报告明确错误，避免重复 subscriber 初始化。
- 默认构建不链接 Tracy；基础/详细档可构建，默认无 GPU 测试通过。实际连接或 capture 验证帧和 scopes，无法运行的环境验证明确记录，不冒充通过。
- 当前文档说明构建、版本匹配、采集、开销、CPU 经过时间与 GPU 时间的区别。
- 非目标：GPU timestamp、全局内存分配跟踪、自动性能优化、并行化、提交策略改动、场景格式改动。不开启完整 Bevy Renderer。

## Result
已实现两档 CPU profiling、统一帧标记、关键渲染/呈现/场景埋点与操作文档。优化构建、默认 26 项测试、真实 GPU 场景生命周期 smoke 和两档 Tracy capture/CSV 检查通过。首次采集暴露的日志前缀过滤问题已修复并复验。未改渲染算法、线程模型或 GPU 同步；尚未做稳定态性能诊断。变更保持 active，未执行 complete/archive。
