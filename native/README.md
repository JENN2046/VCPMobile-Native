# VCPMobile Native: M0/M1 start

这是独立的 Swift/Kotlin ↔ Rust 桥接验证工程，**不是 NativeSync 实现或可安装 App**。旧 `src-tauri/`、Vue 客户端、现有 CI/release 配置不变。

## 已编写的内容

- Rust 共享库：有界输入、结构化错误、有序进度回调、异步等待、可重入取消、单次运行生命周期。
- 12 个 Rust 契约测试；生成真实 UniFFI Swift/Kotlin 绑定。
- Swift/macOS 和 Kotlin/JVM 直接调用 Rust 动态库的合同测试，不用假实现。
- Android arm64 与 iOS device/simulator Rust 目标交叉编译入口。
- 无网络业务调用、无真实用户数据、无凭据、无付费模型、无后台任务调度器。

## 本地命令

在具有 Rust/rustup 的 Linux/macOS 环境执行 `bash native/scripts/check.sh`。Kotlin 测试使用 Java 17 和 Gradle 8.11.1：`gradle -p native/tests/kotlin --no-daemon test`。macOS 执行 `bash native/tests/swift/run.sh`。

`ProbeListener` 回调不是 UI 主线程保证；后续 SwiftUI/Compose 必须正确切回各自 UI 上下文。`cancel()` 只取消本地探针，不取消服务端 AA。每次运行建立新对象；失效对象不能重用。

## 验证层级

Rust 测试、FFI 宿主测试、移动目标编译、SwiftUI/Compose App 编译、模拟器、真机分别记账。此探针即使全部通过，也不能把 M1 标为全部完成，更不能声称 NativeSync R2.1 已接通。

首次依赖解析将打印 Cargo.lock；锁文件在准确结果取得后提交。当前固定直接依赖和 Rust 版本，尚不能宣称传递依赖已完全锁定。

## 后续入口

先补 Compose/SwiftUI 宿主并在各自模拟器调用同一库；同时取得 NativeSync 已接受源码，之后才开展协议适配。准确进度和阻碍见 `STATUS.md`。继承仓库已有 LICENSE，不改变许可或上游署名。
