# 开工状态

日期：2026-10-25。状态：`M0_PARTIAL / M1_STARTED_NOT_VERIFIED`。

本次已获得“安排开工”的明确指示。当前只新增隔离工程；没有自动合并、发布、部署、读取密钥、安装到手机或修改生产数据。

## 准确基线

- VCPMobile main：`88ee3aca0e6a57a9ab830ac4bf380770138f1ed0`。
- 对应树：`89dfe9de975803e07b1de5a0af3400e28a396a0d`。
- 开发分支：`codex/native-rust-m0-m1`，新代码限定 `native/` 和独立工作流。
- 原 Rust workspace 的依赖包括 Tauri、原生插件、SQLite 与网络层；本轮不整包搬运宿主依赖，桥接探针独立 workspace。

## 真实阻碍与处理

1. NativeSync 已接受的 ToolBox commit `a9521bc32c1fb427d89ce6dcc0471c999202375f` 的精确 GitHub 对象读取返回 404。尚未取得对应完整源码/补丁，不能把旧远端代码当 R2.1。M2 的真实适配在准确代码取得前不验收；不阻挡独立 M1 桥接。
2. 当前会话容器未找到 Rust/Cargo/Gradle，网络解析不可用。这里只完成源码与静态检查；编译和测试以实际云端 run 为准。
3. ProjectForge 开放参数定义未提供准确 frozen action 字段/所需 epoch，未猜参数或绕 Remote Desktop 启动 worker。
4. 仓库 Issues 已禁用，创建工作单返回 410；没有更改仓库设置，改用本文件记录。

## 这次工作不是无人值守开发承诺

提交代码与触发 CI 只能证明实际发生的提交和构建。没有成功派发持续编程 agent，不能声称对话结束后会继续自动写完 M0–M5。

## CI 边界

仅现有公开仓库的标准 ubuntu/macOS runner；不改变可见性、不启用大机型、不购买服务；不使用 Actions cache、不上传 artifact、不读取 secrets、GITHUB_TOKEN 仅 contents:read。只在本分支相关文件 push 时或手动调用时运行，不创建 schedule。

## 下一步

- 读取第一轮 CI 的准确 commit/run 结果；修复直接失败，不把排队当通过。
- 提交实际生成的依赖锁文件，固化传递依赖。
- 加 SwiftUI/Compose 应用与模拟器端调用，之后才满足 M1 完成标准。
- 从既有交付位置取得 NativeSync R2.1 精确源码/补丁，核对原生协议及恢复测试后进入 M2。

NativeSync 资料入口：JENN2046/VCPChat #68、#69、#70。它们是交付合同与记录，不等于本次重跑验收或当前运行现场。
