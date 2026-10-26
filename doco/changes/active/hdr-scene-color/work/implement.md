# Implementation design

## 1. Baseline and goals

基线 c74a16c；工作区仅 AGENTS.md 有用户修改。已阅读当前 architecture、asset-runtime，及 overlapping active 包 async-scene-loading / persistent-scenes-and-browser 的 proposal；本次只衔接它们已实现的运行时目标与内存估算，不改变加载/持久化契约。

入口：camera.rs 的 RenderTarget；stage/mod.rs 的 sys_render 和输出复制；systems.rs 的光照与透明绘制；deferred_rendering、transparent；src/material/outline.rs、gizmo；src/editor/gui/mod.rs 的 SDR 纹理注册；src/scene/loading.rs 的相机预算。

原实现每相机每帧切换 A/B，预先创建所有 Stage 的 RenderContext，无法传播阶段内交换；A/B 与输出纹理均为 SCREEN_FORMAT。DeferredComputePipeline 实为 raster 管线；透明注释与实际 MainPassObject 筛选不符，以代码为准。

## 2. Overall approach

保留双 HDR 目标，另外拥有稳定的 SDR display 纹理。Viewport 的 display 与 TargetType::Texture 共享 Arc，不再额外拷贝。窗口目标使用 SDR 中间结果和按实际 surface 格式的 blit 管线。

渲染阶段：Pre → Opaque → PrepareTransparent → Transparent → ToneMapping → PostProcess → Gizmo。Pick 仍在 Opaque 后、Transparent 前；描边 Mask 仍在 Opaque 后、PostProcess 前。PrepareTransparent 复制并交换 HDR A/B。全屏后处理可自行写 alternate 后交换，不需要复制。

每相机排入一个执行闭包，在其中顺序执行所有 Stage；保留每 Stage 一个 encoder/submit。RenderContext 仍按 Stage 创建，但前一 Stage 的 HDR 状态显式传递到后一 Stage。相机全局绑定预建两份，各自采样 A/B；每阶段根据 alternate 选择采样绑定，current 是该阶段写入/最新结果。该机制不添加 CPU/GPU wait，也不在构图时预先决定所有阶段的颜色目标。

## 3. APIs and data model

计划 HdrColorTargets 持有两个 Arc<ColorImage> 和 current 索引，提供 current()/alternate()/swap()，clone 仅复制 Arc 与局部索引。RenderTarget 保留一组纹理；每相机帧创建局部状态，阶段间传递，下一帧重新从 A 开始写入。无历史帧语义。

RenderContext 以 hdr 和 display_target 替换歧义 color_target。RenderTarget.display_color() 返回 SDR 输出；旧 next/get_current_color/get_attachment_color 不再作为对外语义保留，更新仓库调用方。HDR 图像 Rgba16Float（8 bytes/pixel），display 固定 SCREEN_FORMAT，深度/G-buffer 不变。

ToneMappingSettings 是应用级 Resource，默认 exposure_compensation_ev=0；独立 16-byte uniform，不改 CameraUniform ABI 或场景 codec。曝光值仅在上传边界清理非有限输入并限制合理范围，防止 exp2 溢出。ToneMappingPipeline 提供实际生产绘制入口，GPU 验证复用它而非复制 shader 算法。

resize 原子替换 HDR 对、display、depth；TargetType::Texture 同步引用 display，原 resize 事件继续重建 G-buffer、mask 和 egui 注册。旧 egui 注册的延迟释放契约保留。相机预算集中到目标资源估算并保留其他附件余量，活动/候选共用，避免两个魔数独立漂移。

## 4. Algorithms and rules

1. 延迟光照写 HDR current，保持全屏背景覆盖。
2. PrepareTransparent 在自己的 encoder 内复制 current 到 alternate，随后 swap；下个 Stage 才创建其采样绑定选择，因此透明读取原 current、写入原 alternate。透明 Load 保留未覆盖背景，不改变现有排序/折射公式。
3. ToneMapping 读取 HDR current，执行 max(rgb,0) * exp2(EV)，逐分量 Reinhard x/(1+x)，写 sRGB attachment，由硬件编码。alpha 输出 1。
4. 描边/Gizmo 显式写 display，不再依赖隐式目标切换。
5. 删除 IBL 的 2.0 上限，不改变 roughness 等参数约束。默认 LDR cubemap 仍有效。
6. 窗口使用渲染 blit 而非不兼容格式 copy；surface 获取失败跳过并记录。Viewport 不直接把 HDR 交给 egui；保留其 UNORM view 适配，避免双重 sRGB 解码。

GPU 初始化失败沿用现有错误策略；本次不建立新的设备恢复系统。各相机帧状态独立。共享全局 ToneMapping uniform 每帧最多写同一曝光值，不在一个提交内用队列多次覆写它表达多个参数。

## 5. Fixed decisions and discretion

固定：保留 ping-pong、按步骤交换、线性半精度 HDR、独立 SDR、Reinhard/手动曝光、叠加在映射之后、每阶段提交、现有场景格式不变。

可自行决定局部命名、helper 和缓存结构；不引入 Frame Graph 资源编译、异步渲染线程或新依赖。窗口 blit 和示例改用正确呈现路径属于输出格式边界的必要适配。

Blocked: none.

## 6. Verification and documentation impact

无 GPU：WESL 编译/Naga 验证新 shader（已有拾取 shader 测试不覆盖此模块），cargo build，cargo test --workspace --lib --tests，cargo check --examples。

显式 GPU：实际 HDR 数据经生产 ToneMapping pass 后读回 SDR，对照独立参考值验证高亮保留、曝光及 sRGB 编码；生产阶段调度插入确定性 HDR 写入/透明背景观测，验证跨 Stage swap 和连续帧内容；resize 后重新验证。尽量在已有 GPU 验证入口追加独立风险，不新增仅供测试的生产导出。

运行 scene_smoke 检查真实编辑器、加载切换和 resize；视觉检查只作为辅助，不代替像素数值验证。测试门禁：契约为颜色范围/当前帧来源/显示边界；可信回归为 UNORM 截断、过早创建上下文、gamma 重复；现有资产和场景测试不保护像素输出；不增加测试专用 seam。

更新 doco/architecture.md 并新增当前渲染颜色契约 doco/specs/render-color.md；更新 asset-runtime 的目标边界。proposal Result 保持 Pending，不自动 complete/archive。
