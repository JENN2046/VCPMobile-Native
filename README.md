# VCPMobile-Native

SwiftUI（iOS）＋Jetpack Compose（Android）＋UniFFI 共享 Rust 客户端。

本仓库是双原生客户端的独立开发入口。开发分支：`codex/native-rust-m0-m1`。原 `JENN2046/VCPMobile` 保留作旧版维护与来源追溯，本次不修改旧仓库。

## 迁移来源

原生源码来自 `JENN2046/VCPMobile` 提交 `a615bff38e497419c62f1bef1601add6ee19bcca`。`native/` 与 `LICENSE` 的16次开发提交已提取，作者、提交者、原时间和提交说明逐条核对；相关提交ID因历史裁剪而改变，映射见 `docs/migration/COMMIT_MAP.json`。

共59个原范围文件（包括当前自动构建配置）保持逐字节一致。历史构建配置的旧blob和提交关联记录在映射中，完整旧历史保留在原仓库。原 Vue/Tauri 应用、生产配置、真实历史、私有安装ZIP与凭据不纳入本仓库。

保留 `native/` 目录层级，以维持已有构建脚本的相对路径；不依赖旧仓库的 Tauri 工作区。原许可证原文保留，不重新许可。

## 当前状态

**迁移不是产品验收。**M1 曾在旧仓库完成双端模拟器验证；M2仍在施工，界面仍为工程验证界面。当前源版本旧 run `40282422568` 的 Android 任务成功，iOS 有3项UI通过、6项Keychain测试失败（unavailable / -34018）。新仓库需自行构建验证；不能继承旧PASS或把失败隐去。

先读取 `docs/migration/MIGRATION_STATUS.md` 与最新 Actions，再结合 `native/STATUS.md`、`native/WORKLOG.md`。后两者包含旧仓库的历史记录，早期状态不代表当前状态。

## 目录与构建

- `native/core/`：共享Rust客户端、NativeSync协议与恢复逻辑、测试。
- `native/apps/android/`、`native/apps/ios/`：双端原生宿主。
- `native/scripts/`：构建、模拟器及证据脚本。
- `native/Cargo.lock`：已锁定依赖；Rust构建使用 `--locked`。
- `.github/workflows/native-core.yml`：仅开发分支的标准GitHub runner验证。

基础命令：`bash native/scripts/check.sh`。Android、iOS构建要求和命令见 `native/README.md`。不要求本地Mac，iOS可使用云端macOS构建，但模拟器不替代iPhone真机、音频、后台和签名验证。

## 继续开发的边界

继续既定R1.1的M0–M5范围，复用NativeSync R2.1，保留History、OneRing与AA的服务端执行权威。仅在开发分支施工，不自动合并、发布、改生产服务或迁移真实数据。真机操作、签名和额外费用分别处理。

## 来源与许可

原项目：`JENN2046/VCPMobile`（fork），上游 `MRiecy/VCPMobile`。原 `LICENSE` 原文及相关归属保留。NativeSync来源依据见 `native/docs/NATIVESYNC_R21_SOURCE_MAP.md`。公开仓库不表示取消原许可证限制。
