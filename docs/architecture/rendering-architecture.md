# Rendering Architecture

## 状态
- 状态：已完成主链路 MVP
- 阶段判断：主渲染链路已经从“桌面双后端原型”推进到“统一 session + retained/patch 提交”阶段；桌面最成熟，移动端已经具备 attachment、platform presenter builder 与 render session 工厂链路。

## 目标
- 让 Rust 保持框架控制平面。
- 在平台允许时优先走 Impeller 风格路径。
- 在 Impeller 未实现或不可用时回退到 Skia。
- 对上层暴露后端无关的渲染抽象。

## 分层
- `zeno-core` 负责共享类型、配置、平台标识与结构化错误。
- `zeno-text` 负责文本描述、布局契约与 fallback 文本测量。
- `zeno-graphics` 负责 `Scene`、`DrawCommand`、`Renderer` trait 与 backend probe 契约。
- `zeno-runtime` 负责后端优先级、探测、回退选择与初始化策略。
- `zeno-backend-impeller` 与 `zeno-backend-skia` 负责具体后端实现。
- `zeno-shell` 负责宿主窗口、事件循环、surface 生命周期，以及桌面/移动端 render session 创建与 attachment。
- `zeno-compose` 负责声明式节点树、retained tree、最小布局以及从节点树到 `SceneSubmit` 的转换。

## 当前渲染流程
1. Shell 根据当前平台生成 `NativeSurface` 与平台描述。
2. Runtime 读取 `RendererConfig` 并生成后端尝试顺序。
3. 各 backend 根据平台执行 probe，返回可用性与失败原因。
4. Runtime 选出第一个可用 backend，并返回 `ResolvedSession`。
5. Compose 层将声明式节点树翻译成后端无关的 `SceneSubmit`，其中 `SceneLayer` 已可携带 subtree clip / 2D affine transform / opacity / blend / effect / offscreen 状态，`SceneBlock` 负责 layer 内局部绘制命令，并在 paint-only 或局部更新时走 patch 路径。
6. Shell 基于 `ResolvedSession` 创建桌面或移动端 render session，并把 `SceneSubmit` 提交给具体 GPU 或 Canvas 路径。
7. 移动端在进入 render session 前，还会经过 `MobileAttachContext -> MobilePresenterInterface -> platform presenter builder` 的宿主绑定与 presenter 规划过程。

## 已验证能力
- Workspace 已按 `core / graphics / runtime / shell / compose / text / backend-*` 垂直拆分。
- Runtime 已支持 Impeller 优先、Skia 兜底，并能记录每次解析尝试。
- `zeno-compose` 已具备 retained tree、dirty propagation、layout dirty roots 与局部 relayout 路径。
- `zeno-graphics` 已具备 `SceneLayer`、`SceneBlock`、`ScenePatch`、`SceneSubmit` 数据结构，并支持 subtree clip / 2D affine transform / opacity / effect stack 状态。
- Skia 已能消费结构化 scene，具备 dirty bounds 局部提交路径，并已执行 layer 级 blend / blur / drop-shadow MVP。
- macOS 已具备 Impeller Metal presenter，可走桌面窗口渲染路径，并已支持 layer 级 offscreen compositing、blend、blur 与 drop-shadow 执行链。
- 移动端 shell 已具备 `session binding -> attachment -> presenter interface -> render session` 主链路。
- Android/iOS 已分别具备 native-window / view / metal-layer presenter builder，session 不再直接持有通用 renderer。

## 当前仍待补齐
- `Scene` 已进入 retained compositor 的 layer/block 分层阶段；Skia 与 Impeller 都已落地 blend / blur / drop-shadow 执行链，当前主要待补的是更复杂 filter graph、多级 effect 优化与更细粒度 GPU patch。
- Impeller 真局部 GPU 提交仍未完整落地，非 macOS 桌面路径也未完成。
- 移动端 presenter 虽已成型，但 `ANativeWindow / UIView / CAMetalLayer` 到真实 swapchain、drawable、command buffer 生命周期的最后一跳仍未完全原生化。
- 文本主路径已补上 `TextSystem / TextShaper / TextCache` 抽象、glyph 级布局数据与 fallback/system shaping 主干；Impeller 已具备 session 级 glyph cache，但更完整的真实 shaping 覆盖、Skia glyph-run 细化与后端级共享缓存仍待继续升级。
- 工程化验证能力已补上 `examples/text_probe`、scene dump、layout dump；更系统的 bench gallery 与自动化回归工具仍待补齐。

## 为什么保持这种形状
- 选择器放在 runtime，可避免上层直接硬编码 Skia 或 Impeller。
- graphics API 可以在平台策略变化时尽量保持稳定。
- 文本系统独立存在，便于后续单独升级到真实 shaping 和缓存模型。
- shell 与 backend 分离，便于把桌面与移动端宿主能力统一收敛在同一平台集成层。
