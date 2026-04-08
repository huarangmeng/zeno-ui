# SDK Packaging

## 目标

- 对外按“平台 SDK”交付，而不是要求接入方自己拼装 `runtime + shell + backend`。
- `zeno-shell` 跟随平台 SDK 一起发布，因为它承担宿主层职责：窗口、事件循环、surface、session binding、平台 presenter 规划。
- 根 crate `zeno-ui` 作为统一发布入口，内部继续保持多 crate 分层。

## 当前建议的交付层次

### 1. Core SDK

- 组成：`zeno-compose`、`zeno-runtime`、`zeno-graphics`、`zeno-text`
- 用途：给需要自行接宿主层的高级接入方
- 形式：Rust crate 依赖

### 2. Platform SDK

- 组成：Core SDK + `zeno-shell` + 对应 backend
- 用途：给真正要落地到 macOS / Linux / Windows / Android / iOS 的业务方
- 形式：
  - 桌面：Rust crate 或 native library bundle
  - Android：AAR + `.so`
  - iOS：XCFramework

### 3. App-facing Wrapper

- Android：Kotlin / Java facade
- iOS：Swift / Objective-C facade
- 桌面：Rust facade 或上层语言绑定

## 为什么平台 SDK 必须包含 zeno-shell

- `zeno-runtime` 只负责解析 `ResolvedSession`，不接真实宿主对象。
- `zeno-shell` 负责平台 surface、窗口生命周期、事件循环和移动端 session binding。
- 如果平台 SDK 不带 `zeno-shell`，接入方就必须自己实现宿主层，SDK 将退化成渲染内核而不是可直接接入的跨平台 SDK。

## 当前仓库里的打包入口

- 根 crate `zeno-ui` 已配置同时生成 `rlib`、`staticlib`、`cdylib`
- 更直接的平台 preset feature：
  - `macos`
  - `linux`
  - `windows`
  - `android`
  - `ios`
- 底层能力 feature：
  - `desktop`
  - `mobile_android`
  - `mobile_ios`

## 平台产物矩阵

| 平台 | 建议产物 | 对应 feature | 脚本 |
| --- | --- | --- | --- |
| macOS | `libzeno_ui.a`、`libzeno_ui.dylib`、可选 universal 合并产物 | `macos` | `scripts/package-desktop.sh` |
| Linux | `libzeno_ui.a`、`libzeno_ui.so` | `linux` | `scripts/package-linux.sh` |
| Windows | `zeno_ui.dll`、`zeno_ui.lib` / `zeno_ui.dll.lib`、可选 `libzeno_ui.a` | `windows` | `scripts/package-windows.sh` |
| Android | `zeno-ui-android.aar`，内部带 `jni/<abi>/libzeno_ui.so` | `android` | `scripts/package-android.sh` |
| iOS | `ZenoUI.xcframework`，内部带各 target 的 `libzeno_ui.a` | `ios` | `scripts/package-ios.sh` |
| 全量 | 顺序调用 macOS / Linux / Windows / iOS / Android 打包 | 按平台分别启用 | `scripts/package-all.sh` |

## 使用方式

### 桌面

```bash
bash scripts/package-desktop.sh
```

可选环境变量：

- `PROFILE=release|dev|custom`
- `FEATURES=macos`
- `ZENO_DESKTOP_TARGETS=aarch64-apple-darwin,x86_64-apple-darwin`

输出目录：

- `dist/desktop/<target>/`
- `dist/desktop/universal-macos/`

### Linux

```bash
bash scripts/package-linux.sh
```

可选环境变量：

- `PROFILE=release|dev|custom`
- `FEATURES=linux`
- `ZENO_LINUX_TARGETS=x86_64-unknown-linux-gnu,aarch64-unknown-linux-gnu`

输出目录：

- `dist/linux/<target>/`

说明：

- 该脚本面向 Linux 平台原生 SDK 产物，适合在 Linux runner 上执行。
- 若你需要跨端发布，建议在 CI 中单独跑 Linux job，而不是依赖 macOS 本机交叉编译。

### Windows

```bash
bash scripts/package-windows.sh
```

可选环境变量：

- `PROFILE=release|dev|custom`
- `FEATURES=windows`
- `ZENO_WINDOWS_TARGETS=x86_64-pc-windows-msvc,x86_64-pc-windows-gnu`

输出目录：

- `dist/windows/<target>/`

说明：

- Windows 产物建议在 Windows runner 上生成，尤其是 `msvc` 链路。
- 脚本会尽量收集 `zeno_ui.dll`、导入库 `zeno_ui.dll.lib` / `zeno_ui.lib`，以及可用的静态库产物。

### Android

```bash
bash scripts/package-android.sh
```

前置依赖：

- `cargo-ndk`
- `ANDROID_NDK_HOME`
- `jar`
- `zip`

可选环境变量：

- `PROFILE=release|dev|custom`
- `FEATURES=android`
- `ZENO_ANDROID_ABIS=arm64-v8a,armeabi-v7a,x86_64`
- `AAR_NAME=zeno-ui-android.aar`

输出目录：

- `dist/android/zeno-ui-android.aar`

### iOS

```bash
bash scripts/package-ios.sh
```

前置依赖：

- `xcodebuild`
- 已安装对应 Rust target

可选环境变量：

- `PROFILE=release|dev|custom`
- `FEATURES=ios`
- `FRAMEWORK_NAME=ZenoUI`
- `ZENO_IOS_TARGETS=aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios`

输出目录：

- `dist/ios/ZenoUI.xcframework`

### 一次性打包

```bash
bash scripts/package-all.sh
```

行为：

- 始终尝试桌面打包
- 当 `ZENO_PACKAGE_LINUX=1` 时额外打 Linux 包
- 当 `ZENO_PACKAGE_WINDOWS=1` 时额外打 Windows 包
- 在 macOS 且具备 `xcodebuild` 时打包 iOS
- 在具备 `cargo-ndk + ANDROID_NDK_HOME + jar + zip` 时打包 Android

## 平台接入建议

### 桌面

- 若 SDK 用户本身就是 Rust 应用，优先直接依赖 `zeno-ui` crate。
- 若需要给其他语言使用，再在 `cdylib` 之上补一层稳定 C ABI。
- macOS、Linux、Windows 建议分别按平台发独立 bundle，而不是试图用一个桌面产物覆盖三端。
- 最合适的工程化方式是做 CI matrix，分别在 macOS / Linux / Windows runner 上产出对应 SDK。

### Android

- 当前脚本输出的是 native core AAR。
- 更推荐在其上补一层 Kotlin facade，再暴露给业务 App。
- 后续可扩展：
  - `JNI_OnLoad`
  - 生命周期绑定
  - `Surface` / `ANativeWindow` 接入
  - Compose / View 宿主桥接

### iOS

- 当前脚本输出的是 native core XCFramework。
- 更推荐在其上补一层 Swift facade，再暴露给业务层。
- 后续可扩展：
  - `UIView` / `CAMetalLayer` 接入
  - Swift API 包装
  - 线程与生命周期约束收敛

## 当前限制

- 仓库目前已经具备平台 shell 与移动端 session binding 骨架，但尚未完成最终可商用的 Kotlin/Swift 高层 API。
- 目前的 Android AAR 与 iOS XCFramework 主要是 native core 产物，适合先打通 CI/CD 和产物分发链路。
- 如果要面向外部 SDK 用户正式发布，下一步应补齐稳定 FFI / JNI / Swift facade，而不是让业务方直接调用 Rust 内部符号。
