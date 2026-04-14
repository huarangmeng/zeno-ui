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
- Android：计划桥接 `Surface`/`ANativeWindow`；当前已具备 session factory、attach context 与 native-window presenter builder。
- iOS：计划桥接 `UIView`/`CAMetalLayer`；当前已具备 session factory、attach context 与 view/metal-layer presenter builder。

## 当前实现
- `zeno-platform` 根层现在只保留稳定的宿主基元导出：`MinimalShell / NativeSurface / Shell / PlatformDescriptor`；desktop/presenter/session/mobile 改为显式子模块入口，不再在 crate 根平铺大批 re-export。
- 模块文件名已按职责落到 `desktop.rs / presenter.rs / session.rs / window.rs / mobile.rs / desktop_session.rs / platform.rs`，不再依赖一层层无语义的 `mod.rs` 外壳。
- `MinimalShell` 是跨平台 fallback，只生成 `NativeSurface`（用于配置与 backend 解析），不创建实际窗口。
- `DesktopShell::create_surface` 当前复用 `MinimalShell`，因此不会在此阶段创建 winit 窗口。
- `DesktopShell::run_pending_display_list_window` 在启用 `desktop_winit` 时创建 winit 窗口与事件循环，并基于统一 `ResolvedSession` 规划桌面 presenter。
- `DesktopShell::run_animated_scene_window` 现在是 runtime 持有的 `AppHost/run_app` 所消费的底层 host API：shell 只接收 `DisplayList + FrameRequest` 级别的内部协议，不再对 app 层直接暴露入口。
- 桌面输入当前已统一收敛为 `PointerState` 并写入 `AnimatedFrameContext`，窗口层负责采集鼠标位置/按压边沿并触发单次重绘，而不是依赖空转 redraw 驱动交互。
- `MobileShell::prepare_app_session / bind_session` 已能基于统一 `ResolvedSession`、viewport 与 backend 规划移动端 session 绑定结果。
- `MobileShell::attach_session / prepare_attached_app_session` 已引入 `MobileAttachContext`，把 Android `ANativeWindow`、iOS `UIView/CAMetalLayer` 的宿主交接抽象成统一 attach 骨架。
- `MobileShell::create_render_session / prepare_render_session` 已可把 attached session 转成真实 `RenderSession` 对象，形成与桌面一致的 `descriptor -> attach -> session build` 链路。
- Android/iOS 的 presenter 创建接口已经固定到 `MobilePresenterInterface`：区分 `AndroidSkiaNativeWindow`、`AndroidImpellerNativeWindow`、`IosSkiaView`、`IosSkiaMetalLayer`、`IosImpellerMetalLayer` 五类构建入口。
- `platform::android` 与 `platform::ios` 现在分别承载具体 presenter builder，移动端 session 不再直接持有通用 renderer，而是通过平台 presenter 适配层提交场景。

## 当前限制
- `NativeSurface` 已扩展为 `RenderSurface + PlatformDescriptor + target_backend + host_requirement + host_attachment`：shell 现在能在 surface 层表达“需要 desktop window / ANativeWindow / UIView / CAMetalLayer 哪类宿主”，移动 attach 后也会把已绑定宿主回填到 surface。
- 桌面 presenter 初始化已不再完全脱离 `create_surface`：窗口运行链路会先构造带宿主要求的 `NativeSurface`，再把它传给 `DesktopSessionPlan` 与真实 session builder，避免 surface 与 presenter 创建继续分叉。
- 桌面输入分发目前已覆盖 pointer move / left-button press/release，并能参与按需重绘调度；键盘、滚轮、触摸与更高层命中测试仍未进入统一协议。
- 移动端虽然已具备平台 presenter 适配层，但 Skia GLES / Skia Metal 目前仍通过 backend renderer 完成提交，尚未持有真实 swapchain / command buffer / drawable 生命周期。

## 下一步
- 继续把 `host_attachment` 从“原生宿主标识与句柄”推进到真实 swapchain / drawable / command-buffer 生命周期对象，尤其是移动端 GPU presenter。
- 把当前 `PointerState + FrameRequest` 桌面协议继续扩展为统一输入与事件分发契约，避免上层 UI 与具体窗口系统强绑定，并为 gesture / focus / keyboard 生命周期铺路。
- 增加平台能力报告：区分 build-time 支持、runtime probe 与 presenter/session 可创建性，避免平台矩阵过度乐观。
