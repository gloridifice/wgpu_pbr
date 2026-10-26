# Tracy CPU 抓帧

## 构建与版本

默认构建不启用 Tracy。使用优化且保留调试符号的专用 profile：

```powershell
cargo run --profile profiling --features profile-tracy -- --scene scenes/demo.scn.ron
# 加上 wgpu / egui / epaint 等依赖已有的 CPU scopes：
cargo run --profile profiling --features profile-tracy-deps -- --scene scenes/demo.scn.ron
```

`profile.profiling` 继承 release，`debug=2`、`strip=false`；普通 `cargo run` 仍是未优化构建。当前锁定 tracing-tracy 0.11.4、tracy-client 0.18.4、tracy-client-sys 0.28.0，对应 Tracy GUI/capture 0.13.1。更新依赖时检查 `cargo tree --features profile-tracy-deps -i tracy-client-sys`，按 rust_tracy_client 兼容表同步工具；不要假设不同协议版本能连接。

## 生命周期与过滤

`WgpuPbrPlugin` 在其他插件之前调用 `lentille_core::profiling::init`，App 资源持有客户端，LogPlugin 的全局 Tracy layer 也持有客户端。worker 命名为 scene-loader，主线程为 main。仅本机连接（only-localhost），按连接采集（ondemand）；没有连接时不保存历史 zones。Bevy 自带的“未连接时内存增长”警告未识别 ondemand，不表示此构建关闭了 ondemand。

默认只由 LogPlugin 安装一次 subscriber。嵌入方若已安装 subscriber，插件不覆盖；嵌入方需自己安装兼容的 TracyLayer，否则只有直接 Tracy/依赖埋点，没有业务/ECS tracing spans。自建 wgpu App 若开启详细档，必须在调用依赖埋点之前调用 core 的 profiling::init；它只启动客户端，不安装 subscriber。

Tracy 构建明确拒绝 `-t` 并退出 2；非 Tracy 构建保留旧 Chrome tracing 的 `-t`，不再重复初始化 LogPlugin。

Tracy 模式显式设置 `wgpu_pbr=info`，避免 Bevy 默认的 `wgpu=error` 前缀同时屏蔽根 crate。formatter 关闭 ANSI，避免 tracing-tracy 的 zone 字段名称出现转义字符。`RUST_LOG` 会覆盖插件过滤；抓取时建议先清除它，或至少设置：

```powershell
$env:RUST_LOG = 'info,wgpu=error,wgpu_pbr=info,naga=warn'
```

不要全局开启 trace 日志来“启用抓帧”；基础 CPU scopes 为 info 级，详细档依赖的直接 Tracy scopes 不由日志过滤控制。

## 帧与测量边界

- winit runner 的每次 app.update 产生一个 FrameCPU zone，退出该 zone 后发出一次 frame mark；包括 resumed 首次更新。无论相机数、Stage 数、submit 数或 surface acquire 是否成功，均不重复打帧标记。
- 帧标记间隔包含事件循环与更新之间的空隙，FrameCPU 仅包 app.update；第一帧包含初始化，应与稳定态分开分析。
- Bevy 自带 schedule、system、system_commands 和条件检查 spans 保留；不为了抓帧启用 ECS 多线程。
- RenderCamera/RenderStage 位于实际执行的延迟 commands 闭包内；阶段名称作为字段，不能把只生成命令的 sys_render 当作整个渲染耗时。
- EncoderFinish 和 QueueSubmit 分开计时；场景阶段共用每相机批次，它们位于 RenderCamera 内、各 RenderStage 之外。单相机普通编辑器稳定帧约两次提交，不再是逐 Stage 提交。SurfaceAcquire/SurfacePresent 覆盖编辑器和直接窗口目标。CameraBindGroup 记录每相机绑定准备。
- EguiEndPass、EguiPlatformOutput、EguiTessellate、EguiTextureUpload、EguiBufferUpdate、EguiEncode 分开记录。
- SceneCpuLoad 仅包后台 job 执行，不含 Condvar 空闲等待；SceneUploadPoll 记录主线程上传推进，SceneCommit 记录候选最终提交。SceneSnapshotCheck 记录 dirty 缓存失效检查，SceneSnapshotBuild 记录实际反射提取和编码；没有持久变化的稳定帧不应出现后者。

所有上述区间都是 CPU 经过时间，可能包含抢占、驱动或同步等待。详细档中的 wgpu scopes 也不是 GPU 执行时间。当前没有 GPU timestamp、分配器跟踪或逐物体埋点，不新增 GPU 等待，不改变提交和渲染顺序。

## 采集与判读

先启动应用，再用匹配版本的 Tracy GUI 连接 localhost，或在另一个终端运行 capture（程序名取决于安装方式）：

```powershell
tracy-capture -a 127.0.0.1 -s 10 -o cpu.tracy
tracy-csvexport cpu.tracy > zones.csv
```

启动峰值和稳定帧分别抓取；稳定态先等场景加载和 shader 预热完成，保持相同场景、窗口尺寸和相机。对比相同优化配置下关闭、基础档、详细档的开销，不用带大量 zones 的详细档 FPS 直接代替正常性能。不要同时挂 RenderDoc。

首先检查 FrameCPU、system、RenderStage 与 Acquire/Submit/Present 的占比，再展开 egui 和依赖内部。zone 长不等于该线程一直运行；必要时用系统采样/上下文切换判断等待，Windows 相关系统功能可能需要管理员权限。显式 zones 不以采样可用为前提。

Tracy 原始文件可能携带源码路径和符号，保存到本地临时目录或 target 下，不提交仓库。帧边界恰好落在 capture 起止处时，工具的帧数和完整 FrameCPU zone 数可能有少量差异，不据此判定重复打标。
