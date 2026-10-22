# Implementation design

## 1. Baseline and goals

用户已授权执行。以下接口描述实施边界；实际公开接口与验证结果见当前源码和 [verification.md](verification.md)。未完成的原生交互验收不计为通过。

### 实际基线

定位基线：Git `a347de2`。创建变更前已有用户修改的 [AGENTS.md](../../../../../AGENTS.md) 以及未跟踪的 [.agents/](../../../../../.agents/)、[doco/](../../../../)，本变更不覆盖它们。[architecture.md](../../../../architecture.md) 仍为初始化说明，[当前 specs](../../../../specs/) 和 [decisions](../../../../decisions/) 尚无当前契约。

以实际源码为准：根 crate 和工作区使用 Rust 2024，Bevy 依赖为 0.18；根依赖已有 `serde` 和 `ron 0.8`。本变更不顺带升级 Bevy，也不依据 AGENTS 中旧版本说明改动运行代码。

| 当前入口 | 现状及改造方向 |
| --- | --- |
| [src/lib.rs](../../../../../src/lib.rs) 的 `SpawnModelCmd` | 每个实例执行 `mesh.upload(world)`；改成创建持久组件，不直接上传共享网格。 |
| [src/lib.rs](../../../../../src/lib.rs) 的 `sys_generate_plane_scene` 等 | 硬编码场景、随机灯光、独立相机/环境启动逻辑；迁移为场景文件及统一控制器。 |
| [asset/load.rs](../../../../../crates/lentille_render/src/asset/load.rs) 的 `Model::load` | 依赖 World/RenderState，解析阶段上传纹理；拆成纯 CPU 导入和 GPU 准备。 |
| [asset/mod.rs](../../../../../crates/lentille_render/src/asset/mod.rs) 的 `Assets<T>`、`Handle<T>` | 存放 Arc，随机 UUID Handle 为 Copy；可复用容器，但不能把 Handle 当文件身份，也不能假设删除实体会驱逐缓存。 |
| [mesh/mod.rs](../../../../../crates/lentille_render/src/mesh/mod.rs) 的 `Mesh::upload` | 同时生成几何 Buffer 和材质绑定；分离共享几何与各 primitive 材质。 |
| [mesh/renderer.rs](../../../../../crates/lentille_render/src/mesh/renderer.rs) 的 `MeshRenderer` | 同时持有共享网格和实例 Buffer；改为 GPU 缓存索引与独立对象 GPU 状态。 |
| [material/pbr.rs](../../../../../crates/lentille_render/src/material/pbr.rs) | 纹理是 UploadedImage；覆盖以首个 primitive 为基础；改为纯描述及逐 primitive 合并。 |
| [transform.rs](../../../../../crates/lentille_render/src/transform.rs) | 自定义 parent/children；children 只追加，子位置未乘父旋转/缩放；持久化 parent，重建 children 并修正受支持的 TRS 组合。 |
| [camera.rs](../../../../../crates/lentille_render/src/camera.rs) | Camera 包含派生矩阵，RenderTargetConfig 创建后被删除；配置保留，矩阵/当前尺寸分离到运行时。 |
| [src/editor/gui/mod.rs](../../../../../src/editor/gui/mod.rs) | Pane::Scene 直接使用 available_size，DockState 只有中央 viewport 与两侧面板；增加中央下方浏览面板，保持图像坐标隔离。 |
| [src/editor/gui/components/mod.rs](../../../../../src/editor/gui/components/mod.rs) | 直接编辑现有组件；迁移后只编辑持久配置，避免每帧 get_mut 产生假脏状态。 |
| [control/mod.rs](../../../../../src/control/mod.rs)、[control/camera.rs](../../../../../src/control/camera.rs) | Escape 切换全局捕获状态；增加 viewport 焦点边界，保留原快捷键。 |

目标契约见 [资产与运行时](specs/asset-runtime.md)、[场景持久化](specs/scene-persistence.md)、[浏览面板](specs/scene-browser.md)。

## 2. Overall approach

### 模块职责与依赖

- `lentille_render::asset`：资产根、规范化路径、CPU 导入、子资源定位、场景作用域 CPU/GPU 缓存。不得依赖根 crate 的编辑器或场景文件格式。
- lentille_render 中的 mesh、material、camera、skybox、transform 模块：可编辑描述与派生结果的边界、GPU 准备函数、组件更新系统。
- 计划新增 src/scene 模块（mod.rs、codec.rs、controller.rs）：持久组件白名单、文件版本、快照、校验、候选场景与提交、保存和修改保护。
- 计划新增 src/editor/gui/components/scene_browser.rs：条目展示、导航和请求；目录读取可放入相邻独立模块。通过场景控制器操作，不导入 mesh 上传实现。
- [src/editor/gui/mod.rs](../../../../../src/editor/gui/mod.rs)：新增 Pane::SceneBrowser、默认 Dock 分割、打开/保存入口和状态展示。

依赖方向：编辑器 → scene → render；render → core/math/wgpu_utils。文件扫描与场景 codec 不要求 GPU。准备系统不要求知道加载来自启动、菜单还是双击。

### 主数据流

```text
文件 / 编辑器 / 程序化构建
        ↓
序列化组件（用户配置、相对资产引用、实体关系）
        ↓             AssetRoot → CPU 导入缓存
准备与失效系统                  ↓
        ↓               GPU 共享资产缓存
运行时组件（GPU 索引、实例状态、派生矩阵、加载状态）
        ↓
现有渲染阶段
```

浏览器只发送 `SceneRequest`。保存反向提取持久白名单，不从 GPU 或 PreparedMaterial 推断用户配置。

## 3. APIs and data model

以下签名是计划边界；具体错误枚举分支和内部容器可局部调整，职责不可合并回 World 依赖的文件解析。

### 3.1 资产定位与准备

```rust
AssetRoot::resolve(&self, path: &AssetPath) -> Result<PathBuf, AssetError>;
import_model(path: &Path) -> Result<ImportedModel, AssetError>;
SceneAssetCache::request_mesh(&mut self, source: &MeshSource) -> Result<MeshAssetId, AssetError>;
prepare_mesh(mesh: &MeshData, device: &wgpu::Device) -> Result<GpuMesh, AssetError>;
prepare_material(desc: &ResolvedMaterialDesc, ctx: &GpuPrepareContext)
    -> Result<PreparedMaterial, AssetError>;
```

- 保留现有 AssetPath 的资产根相对路径语义，增加规范化、Eq/Hash 与反射适配；shader 路径辅助接口继续工作。
- `AssetRef<T>` 包含路径、受校验的子资源选择器与类型标记。选择器语义为整个文件、Mesh(index)、Material(index)、Image(index)，显示示例为 models/a.glb#mesh/0；磁盘定位不把 `#...` 当真实文件名。
- 图片独立文件用整个文件选择器，glTF 内嵌图片用源模型路径与 image 索引；导入器正确解析 texture 到 source image 的对应关系。
- `MeshSource` 为持久组件，支持 `Asset(AssetRef<MeshAsset>)` 和现有平面的 `Plane { size }` 配方。生成几何参数需要校验，不保存内存顶点 Buffer。
- `ImportedModel` 仅包含 MeshData、CPU 材质描述、图片数据/来源和 primitive 对应关系。内部 MeshData 可由当前 Mesh 改造/重命名；不得夹带 UploadedImage、BindGroup 或 Device。
- `GpuMesh` 只持有顶点/索引 Buffer 和绘制范围。默认材质关系保留在 CPU 资产元数据中，按 primitive 解析；合并多个 primitive 的索引时保持正确顶点偏移。
- `GpuHandle<T>` 为当前作用域内的类型化索引，唯一标识不得在错误作用域复用；查找失败返回 NotReady，而不是访问另一个场景的条目。
- `MeshRenderer` 持有 GpuHandle<GpuMesh> 与 ObjectGpuState。ObjectGpuState 的 Transform Buffer、ObjectBindGroup 属于单一实体，复制实体不能复制该状态。
- CPU 解析键为规范化源路径及实际导入选项；GPU 网格键包括 mesh 选择器；纹理键额外包含颜色空间、GPU 格式和 mip 策略。配方浮点值先校验，再使用稳定编码作为键，不把任意 f32 直接当普通 Hash 键。

缓存由 `SceneAssetScope` 强持有。一个候选/活动场景各有一个 scope，同场景去重，跨场景不共享；全局默认纹理和管线保持现有应用资源寿命。候选失败立即释放其 scope；切换成功后释放旧 scope。GPU 命令和底层 wgpu 引用可能延迟物理释放，但应用不得保留旧 scope。

### 3.2 持久组件与派生结果

| 持久数据 | 运行时结果及规则 |
| --- | --- |
| Transform：local TRS + parent | WorldTransform 与 children 索引；parent 声明实体重映射，children 从父引用完整重建而非仅追加。 |
| MeshSource | MeshRenderer、加载状态；删除来源后删除对应绘制状态。 |
| PbrMaterial 覆盖 | PreparedMaterial（按 primitive）；没有覆盖时使用各自原材质。 |
| Camera：fovy/near/far | CameraState：aspect、view/proj、CameraBuffer；aspect 按目标实际尺寸生成。 |
| RenderTargetConfig：Viewport 或固定 Texture 描述 | RenderTarget/RenderTargetSize；当前 viewport 尺寸不回写持久配置，配置不可删除。 |
| CsmConfig | CSM 纹理、Buffer；尺寸/级联数变化需要重建，而非只更新矩阵。 |
| PointLight / ParallelLight | 上传后的聚合 Buffer；没有全局运行时 Buffer 进入文件。 |
| Environment：默认或贴图来源及预处理参数 | Skybox、预过滤结果、SH；候选先持有独立结果，提交时统一替换。 |
| RotationObject、CastShadow、Pickable、MainPassObject | 对应行为或渲染意图可保存；Selected、PickingID、选择描边和属性窗口不保存。 |

PbrMaterial 不再持有 UploadedImage。标量覆盖为继承/替换；纹理覆盖为 `Inherit | Set(AssetRef<TextureAsset>) | Clear`。全部 primitive 分别合并基础材质，不能继承首个 primitive 的完整材质。运行时合并结果不是第二份可编辑数据。

cgmath 类型不全项目替换。为持久 Vec3/Quat 等提供 remote reflection 适配；Color 使用同一反射适配边界。位置/缩放使用 xyz 字段，四元数使用 cgmath 的命名结构 s（标量 w）与 v（xyz），不采用依赖数组顺序的位置编码。FOV 为度，RotationObject.speed 为弧度每秒，材质/灯光颜色为线性 RGBA。

### 3.3 场景文件与控制器

```rust
SceneCodec::capture(world: &World, instance: SceneInstanceId) -> Result<SceneDocument, SceneError>;
SceneCodec::encode(document: &SceneDocument) -> Result<String, SceneError>;
SceneCodec::decode(text: &str) -> Result<SceneDocument, SceneError>;
SceneController::request(request: SceneRequest);
SceneController::prepare(document: SceneDocument, ctx: &ScenePrepareContext)
    -> Result<PreparedScene, SceneError>;
```

- SceneDocument 内部使用 Bevy 0.18 DynamicScene；SceneCodec 自管外层 RON `(version: 1, scene: ...)` 和反射 seed，不直接使用默认 ScenePlugin 文件加载器。该封装不宣称与原生 `.scn.ron` 字节兼容。
- 场景 RON 使用与所选 bevy_scene 0.18 兼容的版本；若与根 crate 的 ron 0.8 不同，使用独立依赖别名，避免无关改变主题文件读写。具体兼容依赖由解析到的 crate manifest 确认。
- 外层版本先检查，再解析 payload。SceneRootMetadata 组件保存名称和活动相机 Entity；Environment 放在该根实体上。没有任意 Resource 快照。
- `SceneOwned(SceneInstanceId)` 是运行时归属，不存入文件；capture 仅提取该实例的实体。反射注册表与持久白名单分开，decode 也检查白名单。初始化时验证所有持久组件已具备反射注册，不能静默漏存。
- SceneRequest 包含 Open(path)、Reload、Save、SaveAs(path)、RetryEntity(entity)。UI 只提供路径与用户决定；控制器统一负责校验、提示和状态。
- 当前状态包含 current_path、instance、saved_baseline、dirty、操作状态和诊断。saved_baseline 是提交或保存成功后的规范化持久快照，实体和组件顺序稳定；实际持久数据可能变化时重新比较，不以 Mut 的变化标记直接判脏。比较发生在内存，不每帧读写磁盘。
- 保存当前配置快照；动画有意改变的局部 Transform 计入修改，GPU 更新、派生矩阵、窗口尺寸不计入。首版不区分独立的 Play World。
- 无路径时使用编辑器路径输入对话框，限资产根内既有目录；自动补齐 `.scn.ron`，非该后缀报错。已有文件二次确认，取消保留当前状态。

### 3.4 浏览器

```rust
enum BrowserEntryKind { Folder, Scene }
struct BrowserEntry { path: PathBuf, display_name: String, kind: BrowserEntryKind }
scan_scene_directory(root: &AssetRoot, directory: &Path) -> Result<Vec<BrowserEntry>, BrowseError>;
is_scene_file(path: &Path) -> bool;
```

- BrowserState 持有当前目录、按目录缓存的条目与读取错误、单选项、搜索词和卡片尺寸。它属于编辑器 Resource，不属于场景。
- is_scene_file 由 scene 路径模块提供，浏览器和 Open/Save 共用；仅检查受支持后缀和普通文件条件，是否有效场景由 codec 决定，扫描时不解析每个场景。
- Pane::SceneBrowser 默认插入中央 viewport 所在 leaf 的下方，不分割整个窗口；首帧按实际中央区域高度初始化浏览面板，默认约占 35%，为默认窗口中的完整卡片预留至少约 280 点（极小窗口受可用高度限制）；分隔条可拖动，后续窗口缩放不覆盖用户调节。Reset Layout 同样创建该结构。
- 用户补充确认 UE Content Browser 样式：保持底部 Dock，局部深灰主题；上方紧凑导航/搜索工具栏，左侧可折叠目录树，右侧可滚动卡片网格，底部条目数、当前场景与卡片尺寸控制。Folder/Scene 由矢量类型图标区分，不依赖字体符号或新增 GPU 缩略图。
- 目录树只扫描已展开的目录并缓存；折叠分支不递归读取。树节点单击导航，卡片单击选择、双击激活。导航时展开目标祖先；当前目录搜索不区分大小写，仅过滤内存条目，导航清空搜索，不扩大资产范围。
- 卡片按可用宽度换列，长名称截断并提供完整路径提示；当前场景标记与文件选择分开。窄窗口可隐藏 Sources，树/网格区域分别滚动。局部样式不修改全局编辑器主题。
- 面包屑、上级、根目录、Refresh 只改浏览目录。双击文件夹导航；双击 Scene 只入队一次 Open，请求处理在 UI 绘制结束后。
- Save/Save As 可放在编辑器 File 菜单，加载结果与未保存状态在面板或统一场景状态区呈现。目录只在首次显示、导航、Refresh 和相关保存完成时重读。

## 4. Algorithms and rules

### 4.1 准备与失效

保留现有渲染 pass 算法，只调整数据来源和准备顺序。配置编辑后统一执行：移除/失效清理 → CPU 请求/导入 → GPU 共享资产准备 → 实体派生状态准备 → Transform/相机/灯光上传 → 渲染。

GPU 准备显式等待 RenderState、公共纹理和布局就绪。结合真实窗口初始化入口设置 RenderReady 状态；不要仅凭 RenderPreparedStartup 名字假设它与 Startup 的先后。场景请求在条件未满足时保留，而不是触发缺 Resource 的 panic。

同一阶段显式 chain/系统集排序，并在下游 Query 消费新组件前应用 deferred commands。触发条件包括配置变化、运行时组件缺失和 Retry；不能只依赖 Added。配置移除使用移除检测或明确编辑命令清理依赖。

准备状态为 Pending / Ready / Failed。记录已处理的来源与配置修订；失败修订不自动逐帧重试。改变来源时先使旧结果不可绘制，成功后安装新结果。原材质缓存共享，实体覆盖结果独立生成；仅改 Transform 不请求网格/纹理。

父子关系先验证无环，再按拓扑顺序计算；父旋转和缩放作用于子位置。首版保留 TRS 表示，按 SP 限制父节点均匀正缩放，避免无声丢弃剪切。

### 4.2 打开事务

1. UI 入队 Open。一个操作未结束时禁用重复激活；未修改时直接处理，已修改时先等待保存/放弃/取消。
2. 校验 root 内路径和真实文件类型，读取并解码。类型白名单、引用闭包、层级和数值校验在 GPU 操作前完成。
3. 用只含持久组件的临时 World 检查映射和组件恢复；不向临时 World 安装 RenderPlugin，不从反射构造触发 GPU 初始化。
4. 准备候选 SceneAssetScope，以及相机、CSM、材质、环境的 GPU 数据。可借用设备和不可变公共资源，不写当前全局环境 Buffer，不注册/替换当前 egui viewport 纹理。
5. 以单个排他提交系统创建新实体并映射引用，过程中不运行其他查询系统。若插入出现可恢复错误，清理候选实体，旧实例仍保留。插入成功后再删除旧实例、切换环境/活动相机、安装准备结果；系统返回时只有一个活动场景。
6. 清除选中对象、待处理拾取和旧属性窗口映射；已在途 GPU 读回安全完成但丢弃旧 scene generation 的结果，不能把旧 picking ID 当作新场景选择。
7. 重新注册 viewport egui 纹理；旧注册延迟到本帧 egui 绘制结束后释放，避免 Update 已生成的 shapes 引用失效纹理；释放旧场景资产作用域。控制器角度从加载相机 Transform 同步，首帧不能覆盖已保存朝向。
8. 捕获新的 saved_baseline，更新路径与 Ready 状态。只有成功后才标记当前场景已改变。

提交前允许同时持有旧、候选两份场景资产，峰值内存可能增加；首版限制一个候选，不预加载目录内全部场景。设备致命错误不承诺回滚。

### 4.3 保存与启动迁移

capture 前检查可保存实体都有完整来源，引用都在该场景内。不把临时生成的渲染子实体自动纳入文件：导入模型时展开的 mesh 节点是持久节点，带 MeshSource；读档按节点恢复，不再次自动展开模型。保存未准备成功的引用仍保留其描述，错误状态本身不保存。

编码结束后用同目录临时文件写入并刷新，采用支持覆盖语义的原子替换；Windows 不可用“先删目标再 rename”冒充原子保存，必要时使用平台 API/成熟库。失败清理临时文件并保留旧文件。成功后更新保存基线并发出刷新通知。

将旧演示场景构建代码转换成一次性迁移/生成工具或数据构建函数，输出检查后的场景文件；随机灯光保存生成后的具体数值。运行时默认只走统一 Open。添加仅使用程序化平面、固定灯光和默认环境的最小场景作为默认启动场景；旧演示内容保存在另一文件供面板打开。公共字体/着色器/默认纹理仍是应用资源，不要求场景文件自包含这些引擎资源。

### 4.4 目录扫描与 UI 输入

使用 read_dir 按需读取当前目录及目录树已展开的目录，按目录缓存；不逐帧扫描或遍历折叠分支。忽略以点开头的条目、临时文件、链接/junction/reparse point。普通目录全部保留，包括空目录；文件仅保留大小写不敏感的 `.scn.ron` 后缀。先目录后文件，以小写显示名及原名作为稳定排序键；文件操作使用真实 PathBuf。

进入目录与打开文件前重新检查元数据和根约束。规范化和真实路径检查不能仅使用字符串 starts_with；目标不存在、不可读或已被替换时返回诊断。当前目录刷新时已删除则退回根目录；根本身不可读则保留错误与 Refresh。

先构建本帧 viewport 图像区域和 UI 输入归属，再执行相机输入/拾取。浏览面板、对话框或其他 UI 获得输入时不执行相机移动和 viewport 拾取。保留 Escape 捕获切换；光标释放后可导航。viewport 尺寸和点击位置始终来自图像 Response.rect；新增面板不能把其高度算入 RenderTargetSize。Resize 后沿用现有像素缩放逻辑，但引用新的实际尺寸。

## 5. Fixed decisions and discretion

### 已固定

- 用户已授权执行；不升级 Bevy，不替换渲染器，不接 BSN loader。实施不意味着自动完成或归档。
- 持久配置为唯一真值，运行时数据可重建；反射能力不等于持久化权限。
- Bevy 0.18 DynamicScene 封装在 SceneCodec 内，版本 1 的 `.scn.ron` 文件包含自有外层版本封装，不使用默认场景 loader 直接读取。
- 资产根相对引用加子资源选择器；GPU Handle 不落盘；场景内共享缓存，实例 Buffer 独立，场景作用域清理。
- 默认中央上下分割的场景浏览面板，首版只列 Folder / Scene；懒扫描、显式刷新、双击统一打开。
- 可恢复的候选加载失败保留当前场景，存在未保存修改必须确认。
- 先支持现有模型 mesh 范围、平面配方和受限 TRS，不扩展到 glTF 动画、任意剪切和跨场景层级。

### 实施者可局部决定

容器内部布局、私有函数拆分、错误枚举命名、图标、面板最小高度、同目录原子替换库、反射包装类型的命名，以及与 bevy_scene 兼容的 RON 依赖别名。不得以这些局部选择改变公开错误语义、路径边界或保存范围。

无阻塞中的核心设计问题。若实际依赖 API 或 GPU 限制与上述选择冲突，先更新设计并说明影响，不静默更换序列化路线或扩大功能范围。

## 6. Verification and documentation impact

- [AR-A1..A5](specs/asset-runtime.md#acceptance-scenarios)：资源共享、重建、错误重试、材质合并和场景寿命。CPU 导入/键规则可无 GPU 验证，实际 Buffer 独立性和绘制结果需 GPU 验证。
- [SP-A1..A5](specs/scene-persistence.md#acceptance-scenarios)：真实临时文件上的保存加载、非法数据拒绝、当前场景保留、未保存保护、最小场景启动。
- [SB-A1..A5](specs/scene-browser.md#acceptance-scenarios)：目录扫描边界由纯文件系统测试拥有；布局、双击路由和输入隔离在真实编辑器中验证。
- 新测试先说明守护的用户行为、可信回归和唯一主覆盖边界；不验证源代码字符串/注册清单，不仅用序列化自比较代替真实磁盘往返，不为了 mock 增加无生产调用者的接口。
- 实施后运行 `cargo build`、`cargo test --workspace`、`cargo run` 的人工 GPU 验收；GPU 不可用时明确标记未验证，不声称完整交付。
- 交付时依据实际实现补写 [architecture.md](../../../../architecture.md)，将三份目标契约中已交付内容同步到 [current specs](../../../../specs/)，必要时修正 AGENTS 中相关过时事实。创建阶段不把目标当作 current，也不修改已有用户改动。
