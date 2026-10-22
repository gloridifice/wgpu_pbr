# Execution tasks

用户已授权执行。已完成实现与自动验证的任务如下；原生桌面交互和最终全量验收仍保留未完成。详情见 [verification.md](verification.md)。

## 1. 资产与持久数据边界

- [x] 1.1 引入可持久化资产引用并拆开 CPU 模型导入
  - Design: [implement.md §3.1](implement.md#31-资产定位与准备)
  - Acceptance: 规范化的根相对路径与 mesh/material/image 子资源可定位；import_model 不依赖 World/Device；保留每个 primitive 的几何范围、基础材质和正确图片来源；现有程序化平面有可重建配方。满足 [AR-2、AR-A3、AR-A4](specs/asset-runtime.md)。

- [x] 1.2 划分持久组件、派生组件和反射注册白名单
  - Dependencies: 1.1
  - Design: [implement.md §3.2–3.3](implement.md)
  - Acceptance: Transform/来源/材质/相机/灯光/环境等配置不包含 GPU 对象；cgmath 有明确反射适配；parent 和活动相机引用声明映射；运行时类型与 SceneOwned 不落盘；注册缺失可诊断。满足 [AR-1](specs/asset-runtime.md)、[SP-1、SP-2](specs/scene-persistence.md)。

## 2. 渲染资源准备

- [x] 2.1 建立场景作用域缓存和独立的网格实例状态
  - Dependencies: 1.1, 1.2
  - Design: [implement.md §3.1](implement.md)
  - Acceptance: 同场景同来源网格只上传一次，GPU Handle 不能跨作用域误用；每个实体的 Transform Buffer 独立；SpawnModelCmd 改为创建描述组件，复制实体不克隆派生 GPU 状态。满足 [AR-A1、AR-A5](specs/asset-runtime.md)。

- [x] 2.2 根据纯材质描述生成逐 primitive 的运行时材质
  - Dependencies: 2.1
  - Design: [implement.md §3.2](implement.md)
  - Acceptance: 每个 primitive 保留自己的基础材质，覆盖支持标量继承/替换和纹理继承/替换/清除；纹理缓存区分颜色空间等准备选项；属性面板只修改持久描述。满足 [AR-A4](specs/asset-runtime.md)。

- [x] 2.3 分离相机、目标、环境和层级的配置与运行结果
  - Dependencies: 1.2
  - Design: [implement.md §3.2、§4.1](implement.md)
  - Acceptance: RenderTargetConfig 不再被消耗；矩阵、viewport 实际尺寸、children 为派生值；父旋转/缩放正确影响子位置；CSM 配置变化触发正确重建；环境候选不提前覆盖活动 SH。满足 [AR-1](specs/asset-runtime.md)、[SP-A1、SP-A2、SP-A3](specs/scene-persistence.md)。

- [x] 2.4 接通配置变化、移除、失败和重试的统一准备流程
  - Dependencies: 2.1, 2.2, 2.3
  - Design: [implement.md §4.1](implement.md)
  - Acceptance: 明确资源就绪条件、系统顺序和 deferred 应用点；删除运行时状态可重建，删除来源会清理结果；失败不每帧重试；仅改 Transform 不上传网格；失效结果不以 Ready 身份绘制。满足 [AR-A2、AR-A3](specs/asset-runtime.md)。

## 3. 场景持久化与生命周期

- [x] 3.1 实现版本化场景 codec、快照和输入校验
  - Dependencies: 1.2
  - Design: [implement.md §3.3](implement.md)
  - Acceptance: 保持 Bevy 0.18，以 DynamicScene 封装 `(version, scene)`；读写 `.scn.ron`，白名单同时用于保存和加载；实现统一后缀判定、引用闭包/层级/参数校验，拒绝未知版本和运行时类型。满足 [SP-A1、SP-A3](specs/scene-persistence.md)。

- [x] 3.2 实现场景候选准备、统一打开和排他提交
  - Dependencies: 2.4, 3.1
  - Design: [implement.md §4.2](implement.md)
  - Acceptance: Open/Reload/启动入口复用控制器；可恢复失败保留旧场景；提交后清理旧实体、选择、属性窗口、读回结果和环境；正确更新 egui 纹理与相机控制器，释放旧 scope。满足 [SP-A2、SP-A3](specs/scene-persistence.md)、[AR-A5](specs/asset-runtime.md)。

- [x] 3.3 实现 Save/Save As、未保存保护和状态 UI
  - Dependencies: 3.1, 3.2
  - Design: [implement.md §3.3、§4.3](implement.md)
  - Acceptance: 通过规范化持久快照判断真实修改，属性面板仅查看不标脏；首次保存选择根内路径，覆盖需确认；同目录临时文件可靠替换，失败保留原文件；切换的保存/放弃/取消流程完整，成功保存通知浏览器。满足 [SP-A4](specs/scene-persistence.md)。

## 4. Viewport 下方场景面板

- [x] 4.1 实现按目录读取的场景条目数据层
  - Dependencies: 1.1, 3.1
  - Design: [implement.md §3.4、§4.4](implement.md)
  - Acceptance: 只列普通文件夹和 `.scn.ron`，包含空文件夹，隐藏其他文件和点目录；支持稳定排序、缓存、导航/刷新、根边界和删除/不可读处理，不跟随链接。满足 [SB-A1、SB-A2、SB-A5](specs/scene-browser.md)。

- [x] 4.2 接入 Dock 面板、双击打开与 viewport 输入隔离
  - Dependencies: 4.1, 3.2, 3.3
  - Design: [implement.md §3.4、§4.4](implement.md)
  - Acceptance: 默认位于中央 viewport 下方，可调高度，Reset Layout 可恢复；提供面包屑/上级/根目录/Refresh；单击选择、双击文件夹导航、双击场景统一打开；显示当前路径和结果；加载期间不重复请求；图像尺寸/拾取不包含面板。满足 [SB-A1、SB-A3、SB-A4](specs/scene-browser.md)。

- [x] 4.3 将浏览器改为经确认的 UE Content Browser 样式
  - Dependencies: 4.1, 4.2
  - Design: [implement.md §3.4、§4.4](implement.md)
  - Acceptance: 固定底部，局部深灰紧凑工具栏、左侧懒加载目录树、右侧自适应卡片网格、类型图标和底部条目数/尺寸控制；提供当前目录搜索、清空、Sources 显隐；保留文件过滤、双击统一打开、错误保护和 viewport 输入隔离。不扩展其他资产或真实缩略图。通过真实 egui 输入、工作区普通测试及实际窗口截图验证 [SB-A6](specs/scene-browser.md)。

## 5. 测试场景数据迁移

- [x] 5.1 提供最小启动场景及迁移后的演示场景
  - Dependencies: 3.2
  - Design: [implement.md §4.3](implement.md)
  - Acceptance: 最小场景不依赖外部模型，旧演示内容有独立文件；随机灯光烘焙为具体值；启动和双击均走同一打开流程，不重复创建硬编码相机、灯光或 mesh 子节点。满足 [SP-A5](specs/scene-persistence.md)。

## 6. 行为验证

- [x] 6.1 添加并运行无 GPU 的契约验证
  - Dependencies: 2.4, 3.3, 4.1, 5.1
  - Design: [implement.md §6](implement.md#6-verification-and-documentation-impact)
  - Acceptance: 用真实临时目录验证过滤/排序/路径边界及磁盘往返；验证实体引用映射、未知类型拒绝、父子环、缺资产、失败不清除旧配置和保存失败保留文件；每个契约指定唯一主要覆盖边界，不复制源代码清单或只断言自身序列化相等。覆盖 [AR-A3、AR-A4](specs/asset-runtime.md)、[SP-A1、SP-A3、SP-A4](specs/scene-persistence.md)、[SB-A2、SB-A5](specs/scene-browser.md) 的 CPU 部分。

- [ ] 6.2 执行真实编辑器和 GPU 场景验收
  - Dependencies: 4.2, 4.3, 5.1, 6.1
  - Design: [implement.md §6](implement.md#6-verification-and-documentation-impact)
  - Acceptance: 验证网格共享但实例独立、材质恢复、环境切换和资源释放；在面板中完成目录导航、双击打开、坏文件失败、未保存确认；拖动分隔条后拾取正确、面板操作不移动相机；记录 [AR-A1..A5](specs/asset-runtime.md)、[SP-A1..A5](specs/scene-persistence.md)、[SB-A1..A5](specs/scene-browser.md) 的结果和未验证项。

## 7. 最终校验与当前文档

- [ ] 7.1 运行总体校验并同步已交付契约
  - Dependencies: 6.1, 6.2
  - Design: [implement.md §6](implement.md#6-verification-and-documentation-impact)
  - Acceptance: 记录 `cargo build`、`cargo test --workspace`、GPU 人工验收和 `git diff --check` 的真实结果；依据实际实现更新 [architecture.md](../../../../architecture.md) 和三份 current specs，不把未实现计划写成事实；检查相关 AGENTS 说明并保留用户已有内容；运行 `doco check persistent-scenes-and-browser`。只有用户另外授权完成阶段后才修改 Result/生命周期状态。

## Verification

- 任务 1.1–5.1、6.1：构建通过，20 项普通测试与 1 项显式 GPU 测试通过。真实 GPU smoke 完成场景切换、未保存取消、保存重载和候选失败回滚；浏览面板真实 egui 输入测试通过。命令、覆盖关系与限制见 [verification.md](verification.md)。
- 任务 4.3：3 项真实 egui 输入测试通过，覆盖卡片选择/双击/忙状态、树导航/搜索/刷新、窄窗口/视图尺寸/保留选择。实际窗口截图已读取确认 UE 式布局与默认窗口首行卡片可读；GPU smoke 再次通过。Verification: cargo build、cargo test --workspace --lib --tests、cargo run --example scene_smoke。
- 任务 6.2：已取得默认/demo 渲染与面板布局截图；原生桌面焦点不可可靠获取，真实鼠标双击、分隔条拖动后拾取及原生对话框确认仍待人工验收。
- 任务 7.1：当前架构/specs 已同步实现，git diff --check 通过；cargo test --workspace 在三个未改动的旧宏 doctest 失败，不记为全量通过。未自动 complete/archive，Result 仍为 Pending。
- Verification: `doco check persistent-scenes-and-browser` 已在执行结束前重跑；机械检查不替代未完成的 6.2/7.1。
