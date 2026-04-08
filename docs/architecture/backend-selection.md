# Backend Selection

## 状态
- 状态：进行中
- 阶段判断：后端优先级、fallback 策略与统一 `ResolvedSession` 描述符已经成立；当前缺口主要收敛为“非 macOS 的 Impeller presenter 尚未实现”与“移动端 presenter 的 GPU 生命周期仍未完全原生化”。

## 选择策略
- 默认偏好是 `PreferImpeller`。
- 当前平台若存在可用的 Impeller 路径，runtime 优先选择 Impeller。
- 若 Impeller probe 失败且允许 fallback，runtime 会自动切到 Skia。
- 若显式强制某个 backend，runtime 会保留该决定，并在不可用时返回结构化错误。
- 根 crate 默认保留后端切换能力，但桌面开窗仍放在宿主侧 opt-in feature 之后。
- `zeno-backend-skia` 默认提供轻量 stub，并通过 `real_skia` feature 引入真实 `skia-safe` 渲染实现。

## 平台矩阵
| 平台 | Impeller 路径 | Skia 路径 | 当前状态 |
| --- | --- | --- | --- |
| Windows | probe 返回未实现 | 策略层可解析，桌面实现以 Skia 为主 | 当前解析到 Skia |
| macOS | 已有 Metal presenter 路径 | 已有真实桌面路径 | 默认优先解析到 Impeller |
| Linux | probe 返回未实现 | 策略层可解析，桌面实现以 Skia 为主 | 当前解析到 Skia |
| Android | probe 返回未实现 | 已有 native-window presenter builder 与 render session 工厂路径 | 当前 Impeller probe 返回未实现 |
| iOS | probe 返回未实现 | 已有 view/metal-layer presenter builder 与 render session 工厂路径 | 当前 Impeller probe 返回未实现 |

## 当前限制
- `ImpellerBackend::probe` 目前只有 macOS 返回 available，其他平台仍返回 `NotImplementedForPlatform`。
- `ResolvedSession` 现在已显式携带 `platform + backend + attempts + frame_stats`，shell 已能在桌面/移动两侧基于统一 session descriptor 规划会话绑定；移动端还新增了 `MobileAttachContext`、固定的 `MobilePresenterInterface`、平台 presenter builder 与 `create_render_session` 工厂。
- macOS 上 runtime 虽然会优先选 Impeller，但 `Renderer` trait 对应的 Impeller 实现仍偏占位，真实桌面渲染依赖 shell 内的 Metal session。
- Skia 作为兜底策略已经成立，但真实 GPU 桌面呈现依然主要由 shell 持有 surface 与上下文。

## 失败分类
- `NotImplementedForPlatform`：理论上存在该后端策略，但当前平台实现尚未提供。
- `MissingPlatformSurface`：shell 未能提供后端所需的原生 surface 类型。
- `MissingGpuContext`：GPU 路径存在，但当前运行环境无法初始化。
- `RuntimeProbeFailed`：发生了未预期的 probe 失败，并携带字符串说明。
- `MobileAttachPlatformMismatch`：移动端 attach context 与当前 session 所属平台不一致。

## 下一步
- 为 Android/iOS 的 platform presenter 继续接入真实 swapchain / drawable / command buffer 生命周期，让当前 renderer-backed presenter 适配层演进为完整原生 GPU presenter。
- 继续补齐非 macOS 平台的 Impeller presenter，实现 probe available 与真实可创建 session 的一致性。
- 持续扩展验证用例，保持默认策略、fallback、强制失败以及 desktop/mobile session 规划逻辑都可回归验证。
