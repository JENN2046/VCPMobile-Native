# 仓库迁移与接续状态

2026-10-26：`SOURCE_AND_HISTORY_IMPORTED / NEW_REPOSITORY_CI_PENDING`。

开发目标为公开仓库 `JENN2046/VCPMobile-Native`，开发分支 `codex/native-rust-m0-m1`。原 `JENN2046/VCPMobile` 及原分支不修改、不删除、不强推。

## 已核验的迁移内容

- 精确源HEAD：`a615bff38e497419c62f1bef1601add6ee19bcca`，源tree：`d5b8cc9317d983a11010b922b4aa14d7a7f43c7a`。
- 原生历史提取：16个原提交，作者、提交者、时间、说明逐条一致；新旧SHA映射见 `COMMIT_MAP.json`。提取范围为 `native/`、`LICENSE`。
- 当前构建配置独立复制，原blob `0181ec68b33269eb3b072f2b4d7a8b8b3dbab0a4`。历史构建配置blob仍可从映射和旧仓库追溯。
- 59个原范围文件的内容未改变，清单见 `SOURCE_MANIFEST_A615BFF.json`。依赖锁、许可证、已提交双端安全存储与测试都包含在内。
- 成功迁移run：`40286916129`；历史暂存提交：`da2f29398dbfbbaa13cf7191e6e0befa063d24f0`。第一次导出失败与针对性修复保留在 `IMPORT_ATTEMPTS.md`。
- 一次性初始化main把原生历史与3次引导提交连接；不是覆盖旧仓库，也不是产品发布。新仓库的写权限导入workflow已移出当前树，普通验证workflow仍为只读权限。

## 构建与产品边界

当前提交仅完成仓库迁移，不修改产品源码。新仓库首轮构建必须读取该提交对应的实际Actions，不能继承旧仓库PASS。

源版本旧run `40282422568`：Android任务成功；iOS 3项UI通过、6项Keychain失败（unavailable / -34018），唯一根因未确认。新仓库优先复现并修复该真实失败，保留所有测试，不通过模拟Keychain成功、跳过测试或放宽权限检查掩盖问题。

M2真实NativeSync配对、原生安全Cookie持久化接线、提交与回执核对、ACK丢失恢复仍待完整验收；M3–M5未完成。模拟器、真实服务、手机设备和签名各自单列。

## 后续执行入口

自动接续仅切到新仓库，重新启用前必须核对新main/dev准确ref及当前树。每轮先看新仓库最新HEAD、Actions、本文、native/STATUS.md、native/WORKLOG.md和SOURCE_MAP；旧状态文件保留为历史，不将其中早期“缺源码”等陈述当成现状。

只在新开发分支更改native/和配套构建/进度文件。main初始化后不自动再写main，不自动PR/merge/release，不访问生产配置/真实历史/正式凭据，不调用付费模型，不写VCP长期记忆。CI运行中不重复触发；遇并发HEAD漂移停止本轮写入。
