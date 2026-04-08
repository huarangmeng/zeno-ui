# Platform Shell

## 状态
- 状态：进行中
- 阶段判断：shell 已经具备“平台描述 + 桌面窗口生命周期 + presenter 启动”的雏形，但 surface 抽象仍偏轻，且 `create_surface` 当前仍走最小实现，桌面窗口创建只在运行窗口时发生。

## 职责
- 创建并描述原生渲染 surface。
- 持有平台元数据，为 runtime 的后端优先级提供依据。
- 把窗口/视图创建与事件循环留在 shell，不把宿主细节塞进 renderer 抽象。

## 平台模型
- Windows：计划桥接 Win32 surface；当前以 Skia 兜底策略为主。
- macOS：计划桥接 `NSView` 与 Metal layer；当前已有 Metal presenter 原型，Impeller 风格路径优先。
- Linux：计划桥接 Wayland/X11；当前以 Skia 兜底策略为主。
- Android：计划桥接 `Surface`/`ANativeWindow`；当前已具备 session factory / viewport 绑定骨架。
- iOS：计划桥接 `UIView`/`CAMetalLayer`；当前已具备 session factory / viewport 绑定骨架。

## 当前实现
- `MinimalShell` 是跨平台 fallback，只生成 `NativeSurface`（用于配置与 backend 解析），不创建实际窗口。
- `DesktopShell::create_surface` 当前复用 `MinimalShell`，因此不会在此阶段创建 winit 窗口。
- `DesktopShell::run_pending_scene_window` 在启用 `desktop_winit` 时创建 winit 窗口与事件循环，并基于统一 `ResolvedSession` 规划桌面 presenter。
- `MobileShell::prepare_app_session / bind_session` 已能基于统一 `ResolvedSession`、viewport 与 backend 规划移动端 session 绑定结果。
- `MobileShell::attach_session / prepare_attached_app_session` 已引入 `MobileAttachContext`，把 Android `ANativeWindow`、iOS `UIView/CAMetalLayer` 的宿主交接抽象成统一 attach 骨架。
- `MobileShell::create_render_session / prepare_render_session` 已可把 attached session 转成真实 `RenderSession` 对象，形成与桌面一致的 `descriptor -> attach -> session build` 链路。

## 当前限制
- `NativeSurface` 目前更多承担“平台描述 + 逻辑尺寸”角色，不携带可用于 backend 直接呈现的原生句柄。
- 桌面 presenter 的初始化依赖运行窗口路径，与 `create_surface` 分离；当前虽然 descriptor 已统一，但原生句柄仍未前置到 surface 抽象里。
- 移动端虽然已可构建 `RenderSession`，但当前 session 仍以统一占位实现为主，尚未真正桥接 `ANativeWindow` / `UIView` / `CAMetalLayer` 到各 backend 的原生 presenter。

## 下一步
- 让 shell 能提供更强的 surface 描述（可选携带原生句柄 / layer / swapchain 相关信息），并把这一模型继续外推到移动端 presenter 创建链路。
- 引入输入与事件分发契约，避免上层 UI 与具体窗口系统强绑定。
- 增加平台能力报告：区分 build-time 支持与 runtime 可用性，避免平台矩阵过度乐观。
