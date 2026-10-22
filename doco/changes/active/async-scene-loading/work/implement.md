# Implementation

## Baseline and goals

当前代码与已讨论方案一致，工作树开始时干净。入口 src/scene/controller.rs 的 sys_scene_requests 调用同步 open_scene；prepare_scene 混合 CPU 导入、GPU 创建、环境与相机准备。asset/runtime.rs 的缓存未命中会调用 import_model/load_image，environment.rs 两次解码同一图片。src/scene/mod.rs 在 PostUpdate、sys_prepare_assets 前驱动控制器。UI 的 request/pending_open 标记仅足以覆盖同步请求。

当前契约见 [场景生命周期](../../../../specs/scene-persistence.md)、[资产运行时](../../../../specs/asset-runtime.md)。当前代码是执行基线，其他 active 包的计划不是本变更需求。

## Overall approach

- 无新异步运行时依赖。单工作线程 + 请求/结果单槽邮箱，结果带单调 LoadId；取消令牌为 Arc<AtomicBool>，进度为共享小状态。取消可替换尚未执行的已取消请求，发布结果时在结果锁内复查取消令牌；后台执行任务归 worker 所有，帧线程仅取走已完成结果，不 join。
- SceneController 保留当前文件与保存流程，增加 loading 状态、busy/read_only 查询和取消请求。保存确认使用已有交互；所有外部场景请求经控制器二次校验。
- 新 scene/loading.rs 管理 CpuLoadedScene、worker 和候选 GPU 状态；controller.rs 保留相机完成与提交。候选 World 仅包含注册白名单的 Send/Sync 组件及派生变换，不放入 NonSend 或活动资源。
- 新 asset CPU 集合和 GPU 准备器是实际加载链路使用的生产接口。按规范化路径导入 Model，按来源准备 RGBA8 像素，按来源及颜色空间上传。先逐 primitive 合并材质，再收集 Set 纹理，避免漏继承纹理、错误读取 Clear 纹理。同步运行时编辑仍可复用原缓存入口。
- GPU 上传采用 COPY_DST Buffer 分段 queue.write_buffer 和纹理行块 queue.write_texture；预算默认 2ms / 8MiB，每步不可抢占。单批在途，通过 queue 完成回调与 Poll 非阻塞推进，回调不接触 ECS。
- 新 CPU Environment 数据一次解码并计算 SH / f16；应用级 Rgba16Float 预过滤管线在渲染资源初始化创建；候选环境按 cubemap face / mip 分步，必要时 tile 限制像素工作量。候选状态不写活动 SH。
- 主线程资源限制校验先于分配。加载配置暴露 CPU、候选 GPU 与合计估算峰值预算，记录旧缓存与候选估算；第三方一次导入内部瞬时峰值仅能在调用后检查，文档明确此限制。
- Ready 后在 PostUpdate 的资产/变换/相机更新前提交；复用实体映射、回滚、选择、拾取 epoch 和 egui 延迟释放。当前保存基线需在映射后重新快照，O(实体数) 不被误标为异步。

## APIs and data model

生产数据通过 CpuLoadedScene / CPU 资产集合移交所有权；GPU 准备器持有独立 SceneAssetCache、上传游标和完成标记。SceneController 对 UI 暴露 busy/read_only/cancel，不暴露 World 给工作线程。进度只含阶段、当前资源、完成量，不复制候选资产。

## Algorithms and rules

Idle -> AwaitUnsavedDecision -> LoadingCpu -> PreparingGpu -> ReadyToCommit -> Idle；未修改时跳过确认。失败与取消返回 Idle，保留诊断。ReadOnly 从请求/确认开始覆盖行为写入、属性面板、相机控制；选择、目录导航和 viewport resize 可用。Save and Open 允许唯一内部保存继续；忙时不接受其他 Save/Retry/Open/Reload。取消不清 dirty，不改变路径，作废编号。取消 CPU 任务退出前新任务至多等待一个，尚未执行的已取消请求可被新请求替换，不生成无限线程。

CPU 进度不伪造总时间百分比；GPU 进度使用实际字节和完成工作项。正常帧继续渲染旧场景。无活动相机的启动阶段显示加载状态，不把空查询视作错误。GPU 阶段取消后已提交命令不可撤回，drop 自有资源让 wgpu 延迟释放。

候选相机目标在靠近提交时按最新 viewport 大小创建；加载中 resize 不修改持久 RenderTargetConfig。最终提交前禁止改变活动路径、缓存、SH、选择或保存基线。

## Verification and documentation impact

测试遵守 test-audit：CPU 集合测试保护实际依赖闭包/格式，不断言内部函数调用；控制器测试保护异步失败、取消和旧状态；真实 GPU smoke 保护跨帧准备、提交/回滚及读写保护，不复制同一契约至各层，不为测试增加无生产调用接口。

运行 cargo test --workspace --lib --tests、cargo build、显式 GPU 测试和 scene_smoke；可用时进行实际窗口截图。记录每个命令真实结果、性能测量与未验证项。更新 doco/architecture.md、specs/scene-persistence.md、asset-runtime.md、scene-browser.md 中已交付事实，不提前更新。保持本包 active，不自动 complete/archive。

## Fixed decisions and discretion

固定语义如上；局部类型命名、等效帮助函数和预算检查实现可自行决定。Blocked: none。
