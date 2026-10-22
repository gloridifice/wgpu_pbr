# Execution tasks

- [x] 1.1 分离 CPU 资产依赖收集、图片转换及 GPU 分块准备
  - Acceptance: 实际按 primitive 合并并收集依赖、路径去重，GPU 准备不读资产文件；网格/纹理按软预算分块且缓存身份不变。对应 proposal 验收 2。
- [x] 1.2 分离环境 CPU 数据与分步 GPU 环境准备
  - Dependencies: 1.1
  - Acceptance: 环境只解码一次，SH/f16 在 CPU 阶段完成，共用预建管线；转换与预过滤分步且不写活动环境。对应 proposal 验收 1、2。
- [x] 2.1 接入有界后台任务、任务编号、取消和预算
  - Dependencies: 1.1, 1.2
  - Acceptance: Open/Reload/启动统一异步，单 worker、不阻塞轮询、有界任务和 GPU 批次；候选失败/取消保留活动状态。对应 proposal 验收 1、3。
- [x] 2.2 实现只读保护、进度 UI 与排他提交
  - Dependencies: 2.1
  - Acceptance: 属性、行为、相机、外部保存/重试均遵循忙状态；目录和选择可用；提交更新路径、基线、环境与 epoch，支持加载中 resize。对应 proposal 验收 4、5。
- [x] 3.1 验证 CPU 契约及真实 GPU 场景生命周期
  - Dependencies: 2.2
  - Acceptance: 运行普通单元/集成测试、构建、显式 GPU 验证，记录跨帧、失败、取消和资源准备证据；未测性能/交互明确列出。对应 proposal 验收 1 至 6。
- [x] 3.2 同步当前文档并执行最终校验
  - Dependencies: 3.1
  - Acceptance: 更新已交付架构和生命周期/资产/浏览器契约；运行 git diff --check 与 doco check async-scene-loading，保持 active / Pending，不自动完成或归档。

## Verification

任务 1.1：`cargo test --workspace --lib --tests` 22 项通过、1 项 GPU 默认忽略；`cargo test -p lentille_render --lib gpu_assets -- --ignored --test-threads=1` 1 项通过，覆盖删除图片源文件后的分块上传、非对齐行宽、颜色空间及提交后运行时资产编辑。`cargo build` 通过。

任务 1.2、2.1、2.2、3.1：`RUST_LOG=wgpu_pbr=info,lentille_render=info cargo run --example scene_smoke` 通过，输出 SCENE_SMOKE_OK。真实窗口/GPU 覆盖默认/演示场景、加载期间快照不变及旧实体保留、运行时 resize、CPU/GPU 候选失败、GPU 阶段取消及随后打开、保存重载与 CSM 重建。浏览器真实 egui 输入测试覆盖忙时阻止文件打开、允许目录导航和取消请求。

Debug 单次观测：演示场景后台 CPU 14.465s，CPU+GPU 合计 18.628s，289 个 GPU 准备批次；上传/环境编码步骤最大 CPU 用时 2.995ms，排他提交 11.183ms。候选估算 CPU 359285264 字节、GPU（含相机）287036876 字节。这不是完整帧耗时、显存占用测量或 Release 性能承诺；相机准备和 GPU 执行各有不可抢占成本。原生鼠标属性修改/相机输入的人工交互尚未验收，不将 egui 输入或 smoke 等同完整原生交互验收。

任务 3.2：已同步 architecture.md、scene-persistence.md、asset-runtime.md、scene-browser.md；`git diff --check`、改动 Rust 文件的 `rustfmt --check --edition 2024 --config skip_children=true ...` 和 `doco check async-scene-loading` 通过。使用 cutty 捕获并读取本次启动的实际编辑器窗口，确认演示场景、材质、环境和浏览面板正常显示；未取得原生加载中进度画面的截图。旧 active 包保持不变；本包仍 active，Result 为 Pending，未执行 complete/archive。
