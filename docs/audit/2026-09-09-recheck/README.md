# 第二轮审计证据

对应 [项目复审报告](../../项目复审报告-2026-09-09.md)。审计的是 2026-09-09 未提交工作区；准确源码指纹见 `review-manifest.json`。业务源码未被审计工具修改。

| 文件 | 用途 |
|---|---|
| review-manifest.json | 被审源码、配置、锁文件与新 EXE 的哈希 |
| frontend-repro.mjs / frontend-results.json | 真实 Vue watcher/composable 调度，IPC 和 storage 替身；网卡草稿、历史记录及 quota 场景 |
| native-repro.mjs / native-fixtures.rs / native-results.json | 从当前 Rust 源码生成临时 Cargo 工程，完整替换 process runner 与 snapshot reader，再运行原事务和回滚函数 |
| process-repro.mjs / process-results.json | 单独提取实际 process runner，使用无害 Rust 父子进程和两秒定时器检查超时；不包含网络修改函数 |
| npm-advisories.json | 本轮 Yarn 公告扫描的精简字段，不复制完整公告正文 |

## 复现

在仓库根目录、前端依赖与 Rust registry 缓存可用的情况下运行：

```powershell
node docs/audit/2026-09-09-recheck/frontend-repro.mjs
node docs/audit/2026-09-09-recheck/native-repro.mjs
node docs/audit/2026-09-09-recheck/process-repro.mjs
```

Rust 工程生成在系统临时目录，使用缓存离线编译，固定本次 serde/serde_json/encoding_rs 版本；不修改产品 Cargo.toml/Cargo.lock。首次运行需要编译少量依赖。脚本不会调用真实网卡查询或 netsh 修改。

native-repro 只替换外部依赖的实现，保留实际事务和回滚逻辑；输出中的命令是记录字符串，没有执行。process-repro 会运行本地无害进程树，其后代只睡眠两秒。两个脚本的验证范围不同，不能将 native-repro 视为原生网络集成测试。

前端对照样本显示首次列表加载期间 snapshot 仍待返回时 `isLoading` 为 true，因此报告没有把“首次加载一定提前解锁”作为缺陷。进程探针最初的 Node 父子进程版本未复现管道等待，最终保留的是能够复现的 Rust 父子进程版本。

## 解释结果

这些脚本记录现状，不是正式回归测试套件。修复后返回值应改变；应另行编写断言正确行为的产品测试。若源码结构改变导致提取边界失败，应更新脚本，不能直接把提取失败当成产品缺陷。

本轮 npm 退出码为 14，去重 27 条 GHSA。Rust 公告未重做全量 OSV 查询，参考上一轮快照并以当前 Cargo.lock 版本对比；报告另记录了 bytes/time/glib 的 Windows target 检查结果。
