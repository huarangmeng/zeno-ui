# Roadmap

## 状态
- 当前阶段：从“可跑通的多 crate 原型”进入“桌面双后端验证 + 架构收敛”阶段。

## Phase 1 已完成
- Workspace 已完成垂直 crate 拆分并能整体构建。
- API 已围绕 shell、runtime、scene 提交与基础文本能力建立。
- Skia 已作为稳定兜底路径存在。
- Compose 层已具备最小声明式节点树与 `Scene` 生成能力。

## Phase 2 进行中
- Skia 已具备真实绘制实现。
- macOS Impeller 已具备 Metal presenter 与基础 scene 绘制原型。
- 桌面窗口生命周期已经统一到 shell/presenter 侧。
- 当前仍未完成的部分：
  - Impeller 的通用 renderer 抽象仍偏占位。
  - 移动端 shell 与 native surface handoff 尚未落地。
  - runtime 与 shell 之间仍缺少统一 render session。

## Phase 3 下一阶段重点
- 引入 retained tree、dirty propagation、局部布局与局部重绘。
- 重构 `Scene`，支持资源句柄、批处理友好命令模型、layer、clip、transform。
- 将文本系统升级为真实 shaping + cache 管线，而不是只依赖 fallback 测量。
- 将帧循环从持续重绘改为按需调度。

## Phase 4 面向完整 UI 框架
- 增加状态驱动与重组模型，逐步靠近真正 Compose 风格 runtime。
- 增加 widget、focus、input、accessibility、theme 等系统。
- 建立 benchmark、scene dump、layout dump、golden image 等工程化验证能力。

## 当前优先级
1. 收敛 runtime / shell / backend 的渲染提交边界。
2. 去掉持续重绘，建立按需帧调度。
3. 为 compose 和 text 引入 retained + cache 体系。
4. 再扩展更复杂的 UI 语义与跨平台宿主能力。
