# Roadmap

## 状态
- 当前阶段：从“桌面双后端验证”进入“统一 session + retained/patch MVP + 平台 preset 收敛”阶段。

## Phase 1 已完成
- Workspace 已完成垂直 crate 拆分并能整体构建。
- API 已围绕 shell、runtime、scene 提交与基础文本能力建立。
- Skia 已作为稳定兜底路径存在。
- Compose 层已具备最小声明式节点树与 `Scene` 生成能力。

## Phase 2 进行中
- Skia 已具备真实绘制实现。
- macOS Impeller 已具备 Metal presenter 与基础 scene 绘制原型。
- 桌面窗口生命周期已经统一到 shell/presenter 侧。
- runtime 与 shell 已完成统一 `ResolvedSession -> RenderSession` 主链路。
- 移动端 shell 已具备 session binding、attachment、platform presenter builder 与 render session 主链路。
- 当前仍未完成的部分：
  - Impeller 的通用 renderer 抽象仍偏占位。
  - 移动端与桌面 Impeller 的真实 presenter / native object 对接仍需继续原生化。
  - Android/iOS presenter 适配层尚未接入真实 swapchain / drawable / command buffer 生命周期。
  - 非 macOS 桌面 Impeller 与真局部 GPU 提交仍未落地。

## Phase 3 下一阶段重点
- 继续细化 retained tree、dirty propagation、局部布局与局部重绘。
- 继续重构 `Scene`，补齐资源句柄、layer、clip、transform 与更高阶缓存友好结构。
- 将文本系统升级为真实 shaping + cache 管线，而不是只依赖 fallback 测量。
- 增加 bench gallery、scene dump、layout dump 与更稳定的 frame stats 观测能力。

## Phase 4 面向完整 UI 框架
- 增加状态驱动与重组模型，逐步靠近真正 Compose 风格 runtime。
- 增加 widget、focus、input、accessibility、theme 等系统。
- 建立 benchmark、scene dump、layout dump、golden image 等工程化验证能力。

## 当前优先级
1. 继续细化增量布局、局部提交与 Scene 高阶结构。
2. 为 text 与 backend 补齐真实缓存与主路径能力。
3. 继续推进移动端 presenter/native object 的真实接入与 GPU 生命周期管理。
4. 再扩展更复杂的 UI 语义、调试工具与跨平台宿主能力。
