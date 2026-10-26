# 渲染颜色与显示输出

## 颜色空间边界

场景光照、天空和透明绘制使用线性 `Rgba16Float` 颜色，保留高于 1 的亮度。G-buffer 仍为 packed `Rgba32Uint`，深度为 `Depth32Float`；场景 HDR 不要求将所有材质纹理和几何缓冲改为浮点格式。

场景颜色完成曝光与色调映射后，输出 `Bgra8UnormSrgb` 的 SDR display 纹理。选择描边合成和 Gizmo 在色调映射之后绘制，不随曝光变化。HDR 显示器输出不在当前能力范围内。

## Ping-pong 与阶段顺序

每个相机拥有独立 HDR A/B 纹理，每帧创建局部索引并从 A 开始。`current()` 表示当前结果，`alternate()` 表示备用目标；完成写入后显式 `swap()`。不以每帧交换来隐式获取历史颜色。

主颜色链路为：

```text
CSM → G-buffer / 延迟光照 → 透明背景准备 → 透明 → ToneMapping
                                                        ↓
                                      描边合成 → Gizmo → SDR 输出
```

透明背景准备复制 current 到 alternate 并交换，使透明阶段采样本帧不透明背景、写入另一张颜色纹理。透明 pass 使用 Load 保留未覆盖像素；同一 pass 不可同时采样与写入同一颜色纹理。普通全屏 HDR 后处理可直接写 alternate 再交换，不要求复制背景。

每个 Stage 的上下文在前一 Stage 执行后创建，HDR 索引跨阶段传递。当前相机全局颜色采样绑定在阶段入口指向 alternate；若一个阶段内实现多个全屏操作，操作自身需绑定其实际输入，不能假设已有全局绑定随 swap 自动改变。各 Stage 保持独立 command encoder 和顺序 queue submit，不在 CPU 上等待 GPU。

拾取在不透明之后、透明之前；描边遮罩在不透明之后、描边合成之前。这些独立分支不构成完整的线性排序。

## 曝光与 ToneMapping

`ToneMappingSettings` 是应用寿命的运行时 Resource，曝光补偿默认 0 EV，正值增加亮度。上传时将有限 EV 限制在 [-16,16]；非有限输入回退到 0 EV。该设置不进入场景文件，也不改变 CameraUniform ABI。

ToneMapping 对场景颜色执行非负限制、乘 `2^EV`，再逐分量执行 Reinhard `x/(1+x)`，输出 alpha=1。shader 输出显示线性颜色，由 sRGB attachment 完成编码，不重复执行手工 gamma。环境反射采样不在映射前限制到 2.0。

默认 LDR cubemap 仍可使用；外部 HDR 环境支持浮点数据。当前 packed G-buffer 不承诺保存 HDR emissive。多层透明/折射正确合成、Bloom、自动曝光及其他色调映射曲线尚未实现。

## 输出与生命周期

Texture / Viewport 目标和 `display_color()` 共享同一张 SDR 输出纹理，不再把 HDR 图像复制进 UNORM 输出。egui 继续使用 display 的线性 UNORM view 读取编码后的字节，保留已有颜色适配。

直接窗口目标先得到同格式的 SDR display，再按实际 surface 格式 blit：sRGB surface 使用硬件编码，普通 UNORM surface 使用呈现 shader 编码。不能用 texture copy 代替不同格式之间的颜色转换。直接窗口呈现和编辑器窗口呈现应各自拥有目标，不对同一 surface 重复呈现。

resize 重建 HDR A/B、display 和 depth，并通过既有 resize 事件更新 G-buffer、描边和 egui 注册。旧 display 的 Arc 不被原地修改，编辑器仍在本帧 UI 使用完旧纹理后释放旧注册。场景切换不持久化 GPU 对象或 ping-pong 索引。

基础相机附件按每像素 40 字节估算（两个 HDR 16 + SDR 4 + 深度 4 + G-buffer 16）。场景加载预算另保留原有每像素 32 字节的辅助资源余量及 CSM 数组大小；这仍是估算，不是包含任意数量描边组的精确 GPU 内存上限。

## 验证入口

- 无 GPU：`cargo test --workspace --lib --tests`，包含 ToneMapping WESL 编译和 Naga 验证。
- GPU 像素契约：`cargo test -p lentille_render --lib gpu_hdr_colors -- --ignored --test-threads=1`，覆盖高亮、曝光、连续帧背景、部分覆盖、两个相机、resize、显示叠加及不同呈现格式。
- 编辑器生命周期：`cargo run --example scene_smoke`，覆盖真实窗口、加载/取消/切换和 resize。
