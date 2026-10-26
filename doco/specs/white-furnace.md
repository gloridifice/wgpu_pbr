# PBR 白炉诊断场景

## 打开与重建

```powershell
cargo run -- --scene scenes/white_furnace.scn.ron
cargo run --example generate_white_furnace
```

也可在 Content Browser 的 scenes 目录双击 `white_furnace.scn.ron`。生成器不需要 GPU 或外部模型，只重建该场景及 `assets/models/white_furnace/`、`assets/textures/white_furnace/` 下的专用资产；执行真实模型导入和场景序列化往返校验。

## 输入

- 环境每个像素 RGBA 为 255，输入线性辐亮度 RGB=(1,1,1)。白色端点不受 sRGB/线性转换差异影响；512×256 经纬图走现有 SH、cubemap 和预过滤链路，不绕过待诊断的 IBL 实现。
- 场景契约要求一个方向光，故保留零强度 Sun；没有点光、地面、投影物、纹理细节或自发光。
- 18 个共享单位球网格，白色不透明基色。从左到右感知粗糙度为 0、0.1、0.25、0.5、0.75、1（实际下限仍由 shader 决定）。
- 从上到下：金属度 1 / F0=1；金属度 0 / reflectance=0.5（F0=0.04）；金属度 0 / reflectance=0（F0=0 对照）。最后一行并不强制关闭 Schlick 掠射反射，不能当作独立的纯 Lambert shader 测试。
- 实体名称包含材质组和粗糙度，可在层级树选择检查。视口过窄时后退相机或扩大面板。

## 读图与边界

先将全局曝光补偿设为 0 EV，并取消物体选择，避免编辑器描边干扰。曝光是运行时全局设置，不随场景保存或重置。

场景保留现有 Reinhard + sRGB 显示链路；线性 1 映射到显示线性 0.5，sRGB 约 0.735（8-bit 约 188），因此白炉背景在屏幕上是灰白而非 255。球体和背景经过同一映射，可直接比较相对亮暗；不要用屏幕值当作 HDR 辐亮度。

无吸收、包含全部散射阶次的参考应与环境同亮。显著亮于背景意味着增能或归一化/IBL 链路错误；暗于背景意味着能量损失，但单次散射微表面模型本身就可能丢失多次散射能量，不能仅凭变暗判定实现错误。球面不同位置同时提供不同 N·V。

该场景覆盖不透明延迟 PBR + IBL，不覆盖直射光或透明前向路径。`cargo run --example white_furnace_probe` 通过真实场景加载和渲染，回读 tone mapping 前 Rgba16Float：检查全部像素有限、非负且不超过 1.02；介电/对照球中心在 1±0.02 内；光滑金属接近 1，粗糙金属保留单次散射损失。它需要窗口和 GPU，不属于默认无 GPU 测试。它是回归检查，不是所有角度、材质和光照的严格守恒证明。

## 当前着色约定

- 视线为逐片元 `normalize(camera.position - world_pos)`；前向/延迟共享感知粗糙度到 GGX alpha 的转换。
- 镜面采用 Schlick F90=1 与 height-correlated Smith GGX；IBL 镜面权重为 `F0*A+B`。
- IBL 漫反射权重为 `base_color*(1-metallic)*(1-(F0*A+B))`，是基于积分镜面能量的 split-sum 分层近似；直射漫反射使用 `(1-metallic)*(1-F)`。不包含 GGX 多次散射补偿，也不宣称该直射近似严格守恒。
- CPU SH 保存正交基下的 radiance 系数；shader 重建时包含基函数归一化和各阶余弦卷积系数 π、2π/3、π/4，再除 π 得到 Lambert 响应。
- 环境 mip 按实际层数映射粗糙度；CSM 仅调制方向光，不影响 IBL 或点光。
- `cargo run --example generate_dfg_lut` 重建 `assets/textures/ibl_brdf_lut.png`：线性 RGBA8，R=A、G=B，横轴 N·V，纵轴由上到下感知粗糙度递增，128×128、每像素 4096 个样本。以非 sRGB 纹理采样。`cargo test --example generate_dfg_lut` 检查近镜面极限、alpha=1 正视时的解析反照率 `1-ln(2)` 和无增能范围。
- 预过滤修正使用 GGX 半球采样的平方根极角与非退化切线基；默认环境缓存版本为 2，旧卷积缓存会失效。
