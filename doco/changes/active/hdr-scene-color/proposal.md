<!-- doco:lifecycle v=1 created-at=2026-10-26T03:42:52Z completed-at=- archived-at=- -->
# HDR 场景颜色链路

## Purpose

当前延迟光照、透明绘制和编辑器叠加共用 8-bit sRGB 目标，高于 1 的场景亮度在输出时截断。现有颜色双缓冲每帧切换，透明背景实际上读取上一帧。将场景计算与 SDR 显示分开，同时保留 ping-pong，按处理步骤交换颜色目标，为后续 HDR 后处理提供正确边界。

## Scope and acceptance

### 范围

- 场景颜色 A/B 使用线性 Rgba16Float；保留 ping-pong，交换由处理步骤显式触发，不再每帧开始盲目切换。
- 不透明光照与天空输出 HDR；透明前复制当前 HDR 至备用目标、交换读写，透明 shader 采样本帧不透明背景。
- 新增独立 ToneMapping 阶段：全局运行时曝光补偿（默认 0 EV）和 Reinhard 映射，输出 SDR sRGB 纹理。shader 不手动重复 gamma 编码。
- 描边合成和 Gizmo 在 ToneMapping 后绘制，egui 继续消费 SDR 输出。窗口呈现按 surface 格式转换，不复制不同格式纹理。
- 去除环境反射的亮度上限截断；保持既有 G-buffer、材质和场景持久化格式。
- 同步 resize、场景切换、显存预算估算、示例和当前文档。

### 验收

1. HDR 中间结果能区分 1、4、16 等亮度；最终 SDR 与已知曝光/Reinhard/sRGB 值一致。
2. 连续帧背景变化时透明采样本帧背景；没有透明覆盖的像素保持背景。
3. ping-pong 交换在阶段间可见，不会同时采样和写入同一颜色纹理；不同相机的状态独立。
4. 曝光只影响场景颜色，不影响描边/Gizmo/UI。新建、resize、场景切换后纹理格式、绑定与尺寸匹配。
5. 无 GPU 测试和构建执行有记录；显式 GPU 验证覆盖真实像素与生命周期，无法执行的验证明确注明。

### 非目标

不实现 HDR 显示器输出、Bloom、自动曝光、AgX/ACES、OIT、多层折射正确合成、HDR emissive 的 G-buffer 扩展、通用资源 Frame Graph 或整体 PBR 修正。不修改其他 active 变更的生命周期或任务；保留用户已有 AGENTS.md 修改。

## Result

Pending — not completed.
