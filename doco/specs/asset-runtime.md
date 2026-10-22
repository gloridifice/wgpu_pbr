# 资产与运行时组件

## 组件边界

持久组件记录用户配置和资产来源；派生组件记录运行结果。完成准备不得删除来源组件。GPU 对象、GPU Handle、WorldTransform、children 索引、准备错误及选择状态不序列化。

MeshSource 支持资产 mesh 子资源和 `Plane { size }` 配方。PbrMaterial 保存覆盖参数和纹理来源；Camera 保存投影参数，RenderTargetConfig 保留目标策略；其矩阵、Buffer、纹理和当前 viewport 尺寸由运行时生成。

## 资源身份

AssetPath 相对于 AssetRoot（默认项目 assets 目录）。AssetRef 包含路径、类型和 File / Mesh(index) / Material(index) / Image(index) 选择器。子资源索引依赖源文件内部结构，不保证源文件重排后的身份稳定。

路径接受平台路径分隔符并进行规范化。读取和保存不能越过根目录，不跟随符号链接或 Windows junction/reparse point。glTF 的外部 buffer/image URI 同样检查根边界；当前不支持百分号编码或远程 URI。

CPU 导入器不依赖 GPU 或渲染 World，保留各 primitive 的基础材质、顶点偏移和纹理 source image 引用。导入范围为 glTF mesh，不还原完整 node 层级、动画或场景实例。

异步打开时，CpuAssets 在后台按规范化路径导入模型，对每个 primitive 合并材质覆盖后收集实际纹理依赖。同一来源共享解码像素，GPU 上传仍区分 sRGB/线性用途。RGBA8 转换及环境图 SH/f16 转换在后台完成，GPU 准备不再次读取场景资产文件。环境图只解码一次。

## 缓存与实例

一个场景作用域内，同一网格共享顶点/索引 Buffer；同一纹理按来源和 sRGB/线性用途缓存。当前纹理准备使用固定单 mip、RGBA8 格式；不支持的导入图片格式给出错误。

MeshRenderer 持有场景作用域 GPU 索引与实例状态。Transform Buffer 和 ObjectBindGroup 每实体独立，实体复制不克隆该运行时组件。其他场景作用域不能使用旧 Handle。

场景作用域强持有缓存，删除单实体不立即驱逐共享资产。切换场景释放旧作用域；默认纹理与公共管线属于应用寿命。显式 Retry 会重建当前场景网格/材质缓存，避免继续使用修改前的源文件。

## 材质覆盖

未提供覆盖组件时使用各 primitive 的原材质。标量/颜色可继承或替换；纹理可 Inherit / Set / Clear。Clear 使用对应默认纹理，不等于继承。只覆盖 roughness 不改变不同 primitive 的原颜色和纹理；移除覆盖恢复原材质。

编辑器只修改持久描述，不把合并后的 GPU 材质作为第二份可编辑数据。

## 准备与错误

新增或变化的配置、缺失的派生结果、显式 Retry 触发准备。配置移除清理对应结果。失败保留来源和可诊断的错误，同一失败配置不每帧重试。未就绪/失败网格跳过绘制。

仅改变 Transform 更新实例 Buffer，不重新上传共享网格。删除 MeshRenderer 后可由 MeshSource 重建。资源准备需要 RenderReady；解析路径和目录列表本身不要求 GPU。

异步候选使用 AssetUpload 和独立缓存，Buffer 分段写入、纹理按行块写入。时间/字节预算为软限制，至少一完整行可在新一帧推进。候选准备禁止缓存未命中时回退到同步导入；提交后恢复活动缓存对同步运行时编辑的正常导入能力。环境转换及预过滤使用应用级管线和分步 tile，候选不提前修改活动 SH。确认/加载期间暂停活动场景资产及环境准备，取消或失败后恢复。

局部 TRS 通过父先于子的无递归计算生成世界变换；父旋转和缩放作用于子位移。场景文件拒绝父子环和缺失父引用。
