# 原生 UI 框架架构规划

## Summary
- 目标是在当前 `zeno-ui` Rust 库仓库内，规划一个以 Rust 为核心、面向 Windows/macOS/Linux/Android/iOS 六端的原生 UI 框架。
- 首版范围聚焦“最小渲染壳”：先完成应用壳、平台接入、渲染后端选择与切换、基础绘制与文本抽象，不直接进入完整控件系统。
- 渲染策略采用“Impeller 优先，无法使用时回退 Skia”的双后端机制，并要求底层具备运行时或初始化期判断与切换能力。
- API 风格采用“分层并存”：底层暴露渲染与场景接口，上层后续可叠加声明式 UI。

## Current State Analysis
- 当前仓库是一个极简 Rust library 项目，清单文件为 [Cargo.toml](file:///Users/bytedance/RustroverProjects/zeno-ui/Cargo.toml#L1-L6)，仅定义了包名、版本与 edition，尚未声明任何依赖。
- 当前代码入口仅有 [lib.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/src/lib.rs#L1-L14)，内容仍为默认 `add` 示例函数与测试，没有现成的窗口、渲染、平台或 UI 抽象。
- 仓库中尚不存在 workspace、平台目录、渲染后端目录、示例程序或设计文档，因此本次计划需要从零定义模块边界、分层关系与演进路径。

## Proposed Changes

### 1. Cargo.toml
- 将当前单 crate 项目改造成 workspace 根清单。
- 保留根项目元信息或转为纯 workspace root，统一承载多 crate 管理。
- 规划新增成员：
  - `crates/zeno-core`：基础类型、错误、平台能力声明、配置。
  - `crates/zeno-graphics`：渲染抽象、场景接口、画布/路径/笔刷/文本接口。
  - `crates/zeno-runtime`：后端探测、渲染器选择、运行时初始化、能力协商。
  - `crates/zeno-shell`：窗口/事件循环/Surface 生命周期抽象。
  - `crates/zeno-backend-skia`：Skia 后端适配层。
  - `crates/zeno-backend-impeller`：Impeller 或平台原生 Impeller 风格后端适配层。
  - `crates/zeno-text`：文本测量、文本布局与字体系统抽象。
  - `examples/minimal_app`：跨平台最小演示入口。
- 这样拆分的原因是把“平台壳”“渲染抽象”“后端实现”“文本系统”解耦，便于六端落地与后续控件层演进。

### 2. src/lib.rs
- 当前根入口会从示例代码替换为 facade 层，重新导出核心能力，作为未来统一对外 API 的聚合点，或在 workspace 化后由 `crates/zeno` 取代。
- 若保留根 crate，则它仅负责稳定导出，不直接承载平台细节与后端实现。

### 3. 新增 `crates/zeno-core`
- 定义跨平台基础类型：`Size`、`Point`、`Rect`、`Color`、`PixelFormat`、`ScaleFactor`、`BackendKind`、`PlatformKind`、`FeatureFlags`。
- 定义通用错误与能力模型：`ZenoError`、`BackendUnavailableReason`、`PlatformCapabilities`。
- 定义配置对象：`AppConfig`、`WindowConfig`、`RendererConfig`、`BackendPreference`。
- 这里会固化“Impeller 优先，Skia 回退”的偏好配置表达方式，为 runtime 层提供可序列化/可测试的输入。

### 4. 新增 `crates/zeno-graphics`
- 定义与具体后端无关的渲染接口：
  - `Renderer`
  - `RenderSurface`
  - `Frame`
  - `Scene`
  - `Canvas`
  - `Brush` / `Stroke`
  - `ImageResource`
  - `TextParagraph` / `TextLayout`
- 定义 capability query：例如是否支持离屏渲染、GPU 合成、路径布尔、滤镜、文本 shaping。
- 接口层不直接暴露 Skia/Impeller 类型，避免上层 API 被某一后端锁死。

### 5. 新增 `crates/zeno-runtime`
- 实现后端探测与选择器 `BackendResolver`。
- 选择策略按以下顺序执行：
  1. 读取用户配置中的 `BackendPreference`。
  2. 判断当前平台是否存在可用的 Impeller 路径。
  3. 若 Impeller 不可用，则评估 Skia 路径是否可初始化。
  4. 记录最终选择结果与原因，供诊断与 telemetry 使用。
- 运行时输出统一的 `ResolvedRenderer`，对上层隐藏具体选择细节。
- 这里是“底层判断切换能力”的核心位置，计划要求把“选择”与“渲染实现”彻底分离，避免业务层写死后端。

### 6. 新增 `crates/zeno-shell`
- 抽象应用生命周期、窗口创建、输入事件、surface 绑定与重绘调度。
- 规划分平台模块：
  - `platform/windows`
  - `platform/macos`
  - `platform/linux`
  - `platform/android`
  - `platform/ios`
- 壳层负责为 runtime 提供可渲染的原生 surface/handle，而不直接决定用哪个渲染后端。
- 允许少量平台桥接代码：
  - Windows：Win32/DirectComposition/ANGLE 或 Vulkan/Metal 适配入口。
  - macOS/iOS：Swift/ObjC 桥接到 CAMetalLayer/UIView/NSView。
  - Android：Kotlin/NDK 桥接到 Surface/SurfaceTexture/ANativeWindow。
  - Linux：Wayland/X11 抽象，后续按能力选择实现。

### 7. 新增 `crates/zeno-backend-impeller`
- 采用“独立封装”策略，不直接把整个框架建模成 Flutter 内部结构，但在渲染资源与 command pipeline 设计上参考 Impeller 的思路。
- 后端接口目标：
  - 初始化 GPU 上下文。
  - 绑定平台 surface。
  - 执行基础图元绘制。
  - 提供文本渲染接入点。
- 平台选择原则：
  - Android/iOS/macOS 若存在成熟的原生或可接入 Impeller 风格路径，则优先走该路径。
  - 若平台当前无可靠 Impeller 实现，则返回 capability failure，由 runtime 自动回退。
- 此 crate 需要显式区分“接口已定义但平台未实现”和“平台可实现但运行环境不可用”两类失败原因。

### 8. 新增 `crates/zeno-backend-skia`
- 作为稳定回退后端，实现与 `zeno-graphics` 对齐的完整绘制路径。
- 目标是为所有六端至少提供一条统一可行的渲染兜底路线。
- Skia 后端同时承担：
  - 基础图元绘制
  - 文本渲染早期实现
  - 截图/离屏渲染/回归测试支持
- 计划中将 Skia 视为功能完备优先的基础后端，以降低首版跨平台风险。

### 9. 新增 `crates/zeno-text`
- 虽然首版是最小渲染壳，但文本已被纳入基础能力，因此需要单独文本抽象层。
- 首阶段目标不是复杂排版引擎，而是先定义：
  - 字体发现与加载接口
  - 文本测量接口
  - 段落/行盒模型
  - shaping/line breaking capability 标记
- 文本实现建议优先复用 Skia 文本能力作为首个落地点，同时为 Impeller 路径预留抽象接口。

### 10. 新增 `examples/minimal_app`
- 提供统一最小示例，验证：
  - 应用启动
  - 窗口或原生 view 创建
  - 后端解析与回退
  - 基础图形绘制
  - 文本绘制
- 示例需支持日志输出当前平台、选中的后端、回退原因和关键能力。

### 11. 新增 `docs/architecture`
- 建议补充文档目录，至少包含：
  - `rendering-architecture.md`
  - `platform-shell.md`
  - `backend-selection.md`
  - `roadmap.md`
- 文档将解释为什么采用分层并存 API、为什么把后端决策放在 runtime、以及如何从“最小渲染壳”演进到完整声明式 UI。

## Assumptions & Decisions
- 已确认首版目标是“最小渲染壳”，不是一次性完成完整控件框架。
- 已确认对外 API 采用“分层并存”，即底层渲染/场景接口先行，上层声明式 UI 后续追加。
- 已确认平台策略为“全平台并行”设计，因此抽象层从一开始就以六端统一建模，而不是只为桌面服务。
- 已确认允许少量平台原生桥接代码，因此计划会显式预留 Kotlin/Swift/ObjC/C++/Win32 接口层。
- 已确认验收偏向“架构蓝图”，因此首轮实现重点应是 workspace 架构、模块边界、能力模型与后端选择机制，而非立即堆叠大量控件。
- 已确认文本能力纳入首版，因此不会把文本完全推迟到未来版本，只是实现深度会控制在基础测量与渲染层。
- 关于 “Impeller 使用平台原生的，例如 Android 有就使用 Android 的进行渲染” 的解释，当前采用如下工程化落地：
  - runtime 把 Impeller 视为优先级更高的一类渲染路径，而不是强制每个平台都实现同一套底层。
  - 各平台可以用最适合该平台的原生 surface/GPU 接入方式承载 Impeller 风格后端。
  - 只要某平台的该路径不可用或未完成，就统一回退到 Skia。
- 当前计划不假设仓库已存在任何第三方库。执行阶段需要先验证社区内可用的 Rust 绑定、FFI 方案与许可证约束，再锁定具体依赖。

## Verification Steps
- 校验 workspace 拆分后，`cargo metadata` 能正确识别所有 crate 及依赖关系。
- 为 `BackendResolver` 设计单元测试，覆盖：
  - Impeller 可用时优先命中 Impeller。
  - Impeller 不可用时自动回退 Skia。
  - 两者都不可用时返回结构化错误。
  - 用户显式指定后端时的 override 行为。
- 为 `zeno-core` 和 `zeno-graphics` 的公共类型与 trait 编译检查最小示例，确保接口环依赖为零或可控。
- 为 `examples/minimal_app` 在各平台建立最小 smoke test，至少验证初始化、surface 建立、清屏绘制、文本绘制与后端日志输出。
- 在 CI 维度，优先保证桌面端编译检查；移动端先建立 target 级别的交叉编译验证和接口完整性检查。
- 在文档层，要求 `backend-selection.md` 明确列出每个平台的“Impeller 路径”“Skia 回退路径”“不可用原因”三类信息，避免后续执行阶段出现选择歧义。
