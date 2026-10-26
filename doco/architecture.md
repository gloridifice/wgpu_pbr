# 当前架构

## 工作区

项目使用 Bevy 0.18 的 bevy_app / bevy_ecs 管理调度和实体，使用 wgpu 29 自制渲染器，不使用完整 Bevy Renderer。所有工作区 crate 使用 Rust 2024。

- 根 crate `wgpu_pbr`：应用组装、场景控制器、编辑器、相机控制及选择描边。
- `lentille_core`：winit 窗口、输入、时间；在首帧前完成窗口观察者与渲染初始化命令，响应 AppExit。
- `lentille_math`：cgmath 别名、Color 与持久化需要的 remote reflection 适配。
- `lentille_render`：资产导入/缓存、组件准备、渲染阶段、材质、相机、灯光、环境与拾取。
- `lentille_wgpu_utils` / `lentille_wgpu_macros`：类型化 GPU 包装与绑定/阶段宏。

## 持久数据与运行时结果

实体上的局部 Transform、MeshSource、PbrMaterial、Camera、RenderTargetConfig、灯光、CsmConfig、Environment 和测试行为参数可以保存；GPU 对象、缓存索引、世界变换、children 索引、加载状态和编辑器选择不能保存。

PbrMaterial 只保存覆盖参数与纹理引用。每个 primitive 以自己的导入材质为基础合并覆盖。CPU Model 不包含 Device、UploadedImage 或 BindGroup；网格上传与材质准备在资源层单独执行。

```text
场景文件 / 编辑器修改
        ↓
序列化组件
        ↓
AssetRoot → CPU 导入 → 场景作用域 GPU 缓存
        ↓
MeshRenderer / 相机与环境派生状态
        ↓
自定义渲染阶段 → viewport / egui
```

共享网格、纹理由 SceneAssetCache 持有，实体 MeshRenderer 使用类型化 GPU Handle。每实体 Transform Buffer 和 ObjectBindGroup 独立。场景切换释放旧缓存作用域，应用级默认纹理、Device 和管线保留。显式资产 Retry 重建当前场景的资产缓存，以重新读取修复后的文件。

## 渲染颜色链路

不透明物体使用 G-buffer + 全屏延迟光照，透明 primitive 使用前向绘制。每相机拥有线性 Rgba16Float 的 HDR ping-pong A/B 和独立 SDR display；帧内显式交换读写目标，Stage 执行闭包传递最新索引，仍按 Stage 独立提交 GPU 命令。透明准备先复制并交换，使透明读取本帧不透明背景而非历史帧。

主链路为 CSM → G-buffer/光照 → 透明背景准备 → 透明 → ToneMapping → 描边合成 → Gizmo。ToneMapping 使用运行时全局曝光补偿和 Reinhard，写 Bgra8UnormSrgb，由硬件编码；编辑器叠加不受曝光影响。当前 `DeferredComputePipeline` 名称仍沿用旧命名，实际是全屏 raster RenderPipeline。

Viewport 的对外纹理与 display 共享，不再从 HDR 做纹理复制。egui 保持原有 UNORM view 适配；直接窗口目标使用匹配 surface 格式的 blit。resize 重建 HDR 对、SDR 与深度，沿用事件更新附属资源。基础相机附件为 40 bytes/pixel；加载预算在其上保留辅助余量和 CSM 估算。详见 [渲染颜色与显示输出](specs/render-color.md)。

## 场景控制器

`src/scene/codec.rs` 使用 Bevy 0.18 DynamicScene 提取实体白名单，负责版本化 RON、实体引用映射、结构与参数校验；不启用默认 Bevy 场景资产加载器。升级到 bevy_world_serialization 尚未实施。

`src/scene/controller.rs` 统一处理 Open、Reload、Save、SaveAs、Retry 和取消加载。`src/scene/loading.rs` 用单个后台线程和有界单槽邮箱完成 CPU 候选 World、校验、世界变换、模型/纹理导入和环境 SH；任务编号及取消令牌隔离迟到结果。`CpuAssets` 与 `AssetUpload` 分离 CPU 依赖闭包和分块 GPU 上传，环境转换/预过滤按 tile 分帧，预过滤管线为应用级公共资源。GPU 候选至多一批工作在途，靠完成回调和非阻塞 polling 推进。

候选全部就绪后由排他系统提交。可恢复的 IO/解析/资源错误或取消不会删除活动场景、改变当前路径或保存基线。提交时同步相机控制器、环境 SH，清理选择与属性窗口；旧拾取读回按场景 epoch 丢弃。确认和加载期间通过控制器只读状态保护属性、CSM、相机及行为写入，暂停活动资产/环境准备；仍可选择、浏览目录和 resize。Save/Retry 仍同步，渲染初始化与最终 O(实体数) 提交仍可产生耗时尖峰。

UI 在 Update 生成，场景操作及资产准备在 PostUpdate 执行，之后更新变换、相机和灯光，Last 中执行渲染。旧 viewport 的 egui 纹理注册延迟到本帧 UI 绘制之后释放，避免切换时引用已释放的纹理。

## 编辑器

`src/editor/gui/mod.rs` 管理 Dock。默认中央 viewport 下方是 UE 式 Content Browser 面板，两侧 Control Panel 和 CSM Preview 独立。面板使用局部深灰主题、懒加载目录树、可见行虚拟化的卡片网格、当前目录搜索和 View 尺寸控制；只展示文件夹与 `.scn.ron`，不展示其他资产或真实缩略图。树与网格共享按目录缓存；导航、展开和显式刷新触发按需读取，双击通过场景请求打开，不自行解析文件或操作 GPU。

属性面板先编辑配置副本，实际变化才写回 ECS。相机控制在 UI 判定 viewport 输入归属之后执行：右键按住飞行、F 聚焦选中子树、Alt + 左键围绕焦点旋转；导航期间捕获光标，释放、失焦或只读时退出。输入层累计原始鼠标位移并记录按钮按住状态，聚焦通过 SceneAssetCache 的只读 CPU 网格访问计算世界包围范围。详见 [视口相机导航](specs/viewport-navigation.md)。浏览目录、搜索与树/卡片视图状态、面板高度不进入场景文件。

## 入口与验证

- 默认加载 `assets/scenes/minimal.scn.ron`；`--scene scenes/demo.scn.ron` 可选择演示场景。
- `examples/generate_scenes.rs` 是场景数据生成工具；运行演示数据生成需要相关模型文件。
- `assets/scenes/white_furnace.scn.ron` 是恒定白环境下的 18 球 PBR/IBL 诊断场景；`examples/generate_white_furnace.rs` 独立重建其全部资产，不依赖外部模型。`examples/white_furnace_probe.rs` 通过真实渲染回读 HDR 做显式 GPU 回归；`examples/generate_dfg_lut.rs` 重建标准 GGX A/B LUT。着色约定、参数和读图限制见 [白炉诊断](specs/white-furnace.md)。
- 普通契约测试位于根 `tests/`、render crate 和浏览面板单元测试，不需要 GPU。
- 显式 GPU 测试与 `examples/scene_smoke.rs` 验证缓存共享、分块准备/释放、异步场景切换、取消、加载中快照保护及 resize，需要 GPU；不属于默认无 GPU 测试。`lentille_render::tonemapping` 的显式 GPU 像素验证覆盖 HDR 高亮、曝光/sRGB、阶段间交换、连续帧背景、相机隔离和不同呈现格式。

功能契约见 [资产与运行时](specs/asset-runtime.md)、[场景持久化](specs/scene-persistence.md)、[场景浏览面板](specs/scene-browser.md)。
