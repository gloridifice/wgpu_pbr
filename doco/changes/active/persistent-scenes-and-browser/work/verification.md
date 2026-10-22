# 执行与验证记录

## 已交付实现

- CPU 模型导入、持久资源引用、场景作用域 GPU 缓存和独立实例 Buffer。
- 纯描述材质及逐 primitive 覆盖；相机/目标/环境配置与派生状态分离；父子 TRS 计算和派生状态重建。
- Bevy 0.18 DynamicScene 的版本化场景文件、输入白名单、引用/参数校验、候选准备后提交、Save/Save As/Reload、未保存修改保护。
- 中央 viewport 下方 UE 式 Content Browser 面板、目录树/卡片网格、本地搜索、视图大小、目录导航/刷新/过滤和统一双击打开入口。
- minimal/demo 场景数据、生成工具、真实 GPU 生命周期 smoke 工具；旧 test_render example 已适配新的组件边界。
- 当前架构及三份 current specs 已依据实现更新。AGENTS 的用户已有 Doco 入口保留；相关构建/场景/测试说明已修正。

## 命令结果

| 验证 | 结果 |
| --- | --- |
| cargo build | 通过；存在原有未使用项/生命周期警告。 |
| cargo test --workspace --lib --tests | 20 项普通测试通过；GPU 测试默认忽略。包括真实临时文件往返、引用映射、非法场景、控制器错误保护、目录过滤和 Windows junction 边界。 |
| cargo test -p lentille_render --lib gpu_assets -- --ignored --test-threads=1 | 1 项显式 GPU 测试通过。覆盖共享网格、实例 Buffer 独立、派生重建、来源移除、失败不自动重试、恢复文件后的显式重试、同图片不同色彩空间和旧作用域释放。 |
| cargo run --example generate_scenes | 通过；生成并解码校验两个场景文件。 |
| cargo run --example scene_smoke | 通过，输出 SCENE_SMOKE_OK；在真实窗口/GPU 中完成 minimal → demo → minimal、未保存取消、保存重载、缺失模型候选失败后的旧场景保留，以及不移动相机时的 CSM 尺寸重建/初始化。 |
| cargo test --workspace | 普通测试通过，但最终返回 101：未改动的 lentille_wgpu_utils/bind_group_macro.rs 有三个不可编译 doctest。缺少宏导入及字面量省略号，非本变更引入；本次不通过忽略/删除它们来掩盖失败。 |
| git diff --check | 通过；仅有 Git 的 LF/CRLF 转换提醒。 |
| doco check persistent-scenes-and-browser | 机械检查通过；仅提示 6.2、7.1 未完成。 |

临时测试输出位于项目忽略的 cache 目录，不作为交付文档依赖。所有由本次验证启动的应用已关闭。

## UI 与场景证据

1. 实际启动默认 minimal 场景并通过 cutty 截图读取：物体和阴影可见；Scenes 位于中央 viewport 下方，两侧面板高度独立；列表显示资产文件夹，当前场景和缓存统计可见。
2. 实际使用 --scene scenes/demo.scn.ron 启动并截图读取：模型可见，当前场景更新，缓存显示 3 个模型、10 个 mesh、12 个纹理。
3. 真实 egui RawInput 事件测试通过：单击只选择、双击文件夹导航、双击场景只入队一次 Open；没有直接 mock 按钮 clicked 返回值或场景控制器结果。
4. GPU smoke 发现并修复了同帧切换时过早释放旧 egui viewport 纹理的问题；改为绘制后退休，再次运行不再出现 Missing texture 警告。
5. 首次启动暴露窗口观察者的 deferred 初始化未在 First 之前完成；补充窗口初始化 flush，实际应用和 smoke 已验证。窗口 runner 也响应 AppExit，诊断工具可以正常退出。
6. 最终复核补充 CSM 初始化状态：重建纹理后即使相机不移动也会写入矩阵，未初始化的 CSM 不参与绘制；FOV/投影变化触发矩阵更新。真实 GPU smoke 已覆盖尺寸重建场景。

## UE 式浏览面板补充交付

用户确认保持底部 Dock、只浏览 Folder / Scene、使用类型图标而非真实缩略图。实现局部深灰主题、上方紧凑导航/搜索工具栏、左侧懒加载目录树、右侧可见行虚拟化卡片网格与底部 View/状态栏。搜索限当前目录，导航清空；目录树与网格共享目录缓存，Refresh/保存通知一起失效。

- 原浏览器单一交互测试扩展为 3 项真实 egui RawInput 测试；主要边界仍是浏览器 UI，不复制已有文件系统过滤测试，不添加测试专用生产接口。
- UI 测试覆盖卡片单击/双击、忙状态阻止打开、树导航、忽略大小写的本地搜索、无匹配与空目录、缓存直到 Refresh 才反映外部变化、Sources 显隐、窄窗口换列与卡片尺寸数字输入。
- 验证中发现 View 菜单默认点击关闭行为使尺寸数字无法编辑；修改为仅点击外部关闭后，同一输入用例由失败变为通过。
- 真实窗口截图已通过 cutty 捕获并读取：目录树、工具栏、金色文件夹卡片和状态栏可见，位于 viewport 下方且不切短左右面板。默认小窗口首行卡片名称完整可见；首次布局依据实际高度分配浏览器空间。
- 最新 cargo build、cargo test --workspace --lib --tests 与 cargo run --example scene_smoke 均通过。cargo test --workspace 重新执行后仍仅在三个既有宏 doctest 失败。
- 本轮窗口消息注入未可靠触发树导航，不将它当作原生鼠标验收通过；3 项 egui 交互测试及截图不替代下面保留的人工验收。

## 尚未完成的验收

当前 Windows 会话中的 SetForegroundWindow 返回 false；虽然能截图并向窗口投递消息，但无法可靠取得原生交互焦点，因此未将这些尝试记为原生鼠标验收通过。

仍需在可交互桌面确认：

- 从面板使用真实鼠标导航、双击打开、刷新及点击损坏/被删除场景。
- 原生 Save As 路径输入和覆盖确认、未保存修改的保存/放弃/取消对话框。
- 拖动中央分隔条后，viewport 拾取准确；在面板中滚动/输入不会移动相机或拾取物体。
- 多次切换时属性窗口与选择可视反馈的完整清理。

因此任务 6.2 与依赖它的最终验收 7.1 保持未完成，proposal 的 Result 保持 Pending，未执行 complete/archive。GPU 与控制器生命周期已有自动验证，但不替代以上原生交互场景。
