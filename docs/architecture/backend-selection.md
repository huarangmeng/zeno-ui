# Backend Selection

## 状态
- 状态：进行中
- 阶段判断：后端优先级与 fallback 策略已经成立，但平台矩阵还不是“真正全平台可用”，目前主要验证的是桌面路径，尤其是 macOS 的 Impeller 原型和 Skia 兜底。

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
| Windows | 规划中，尚未实现 | 已规划兜底 | 当前解析到 Skia |
| macOS | 已有 Metal presenter 原型 | 已有真实桌面路径 | 默认优先解析到 Impeller |
| Linux | 规划中，尚未实现 | 已规划兜底 | 当前解析到 Skia |
| Android | 概念存在，尚未实现 | 仅策略层存在 | 当前 Impeller probe 返回未实现 |
| iOS | 概念存在，尚未实现 | 仅策略层存在 | 当前 Impeller probe 返回未实现 |

## 当前限制
- `ImpellerBackend::probe` 目前只有 macOS 返回 available，其他平台仍返回 `NotImplementedForPlatform`。
- macOS 上 runtime 虽然会优先选 Impeller，但 `Renderer` trait 对应的 Impeller 实现仍偏占位，真实桌面渲染依赖 shell 里的 Metal presenter。
- Skia 作为兜底策略已经成立，但真实 GPU 桌面呈现依然主要由 shell 持有 surface 与上下文。

## 失败分类
- `NotImplementedForPlatform`：理论上存在该后端策略，但当前平台实现尚未提供。
- `MissingPlatformSurface`：shell 未能提供后端所需的原生 surface 类型。
- `MissingGpuContext`：GPU 路径存在，但当前运行环境无法初始化。
- `RuntimeProbeFailed`：发生了未预期的 probe 失败，并携带字符串说明。

## 下一步
- 将“runtime 解析 backend”和“shell 再按 backend 分发 presenter”收敛到统一的 render session 抽象。
- 让平台矩阵只描述真实可用状态，不再提前写入尚未打通的移动端能力。
- 为 resolver 增加更清晰的验证用例，覆盖默认策略、fallback 和强制失败三类场景。
