# 审计证据说明

日期：2026-09-09；基线提交：`2c6fdafbfb46aca96e68aa8eaffc5ea728f3d4de`。

- [审计文档](../../项目审计文档-2026-09-09.md)：缺陷、定位、风险条件与检查记录。
- [重构报告](../../项目重构报告-2026-09-09.md)：架构、技术选择、实施阶段和验收要求。
- `npm-advisories.json`：Yarn 查询结果的字段投影，按 GHSA 去重；未保存公告完整正文。39 个路径等告警计数、27 条唯一公告。
- `rust-osv.json`：Cargo.lock 中 409 个 registry 包版本的 OSV 批量查询命中摘要，未按 target/feature 筛选；不是 cargo-audit 输出。
- `reproduce-observations.mjs` / `frontend-observations.json`：提取实际 App.vue 脚本并在模拟 IPC/storage 的环境复现指定行为。需要按锁文件安装前端依赖。
- `parse-injection.ps1` / `injection-ast.json`：从实际 Rust 源码提取 PowerShell 模板，进行纯 AST 解析，不执行模板。

本文中的复现脚本不操作本机网卡、不启动 Tauri，不等同于完整原生回归测试。修复代码后复现结果发生变化是预期现象。

## 重新采集

从仓库根目录运行：

```powershell
node docs/audit/2026-09-09/reproduce-observations.mjs
pwsh -NoProfile -File docs/audit/2026-09-09/parse-injection.ps1
corepack yarn audit --json
```

OSV 原始方法：解析 Cargo.lock 中 registry 包的名称与版本，以 `package.ecosystem = crates.io` 构造查询，向 `https://api.osv.dev/v1/querybatch` 提交；按响应顺序映射到包，保留有 `vulns` 的条目。没有上传源码。首次网络失败的空输出已舍弃，保存的是成功重试结果。数据库会变化，未来重新扫描不保证与此快照一致。
