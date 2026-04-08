# Rendering Architecture

## 状态
- 状态：已完成基础分层
- 阶段判断：主渲染链路已经成立，但当前仍是以桌面原型验证为主，尚未进入 retained tree 和高性能增量刷新阶段。

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
- `zeno-shell` 负责宿主窗口、事件循环、surface 生命周期与桌面 presenter 启动。
- `zeno-compose` 负责声明式节点树、最小布局以及从节点树到 `Scene` 的转换。

## 当前渲染流程
1. Shell 根据当前平台生成 `NativeSurface` 与平台描述。
2. Runtime 读取 `RendererConfig` 并生成后端尝试顺序。
3. 各 backend 根据平台执行 probe，返回可用性与失败原因。
4. Runtime 选出第一个可用 backend，并返回解析结果。
5. Compose 层将声明式节点树翻译成后端无关的 `Scene`。
6. Shell 根据后端类型启动对应桌面 presenter，把 `Scene` 提交给具体 GPU 或 Canvas 路径。

## 已验证能力
- Workspace 已按 `core / graphics / runtime / shell / compose / text / backend-*` 垂直拆分。
- Runtime 已支持 Impeller 优先、Skia 兜底，并能记录每次解析尝试。
- Skia 已能把 `Scene` 翻译为真实 Canvas 绘制命令。
- macOS 已具备 Impeller Metal presenter 原型，可走桌面窗口渲染路径。

## 仍然缺少的关键层
- 保留式 UI 树与脏标记传播。
- 局部布局、局部重绘与资源缓存。
- 统一的 render session 抽象，避免 runtime 和 shell 双重按 backend 分发。
- 真实文本 shaping、glyph cache 与 paragraph cache。

## 为什么保持这种形状
- 选择器放在 runtime，可避免上层直接硬编码 Skia 或 Impeller。
- graphics API 可以在平台策略变化时尽量保持稳定。
- 文本系统独立存在，便于后续单独升级到真实 shaping 和缓存模型。
- shell 与 backend 分离，便于将来把桌面验证结果迁移到移动端宿主实现。
