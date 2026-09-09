# 第三轮修复核验证据

对应[第三轮核验报告](../../审计修复核验-2026-09-09.md)。此目录由本轮复核新增，保留旧审计证据。

- `review-manifest.json`：当前业务源码、锁文件、配置与 EXE 指纹，标明是否与上次相同。
- `frontend-repro.mjs` / `frontend-results.json`：实际 Vue composable 与调度；IPC、storage 为替身。
- `native-repro.mjs` / `native-fixtures.rs` / `native-results.json`：保留实际事务与回滚源码，完整替换执行器和快照查询；没有真实网络操作。
- `process-repro.mjs` / `process-results.json`：单独提取实际执行器；本地 Rust 父子进程仅等待并写测试完成标记，验证超时后后代存活。
- `snapshot-probe.ps1` / `snapshot-results.json`：提取固定快照脚本，用 mock cmdlet 制造路由失败；仅为注入测试输入去掉 Console 编码赋值，未读取/修改真实网络和注册表。
- `build.log` / `dist`：本轮前端构建日志及输出，构建期间 stats.html 按原字节恢复。

从仓库根目录运行：

```powershell
node docs/audit/2026-09-09-final-recheck/frontend-repro.mjs
node docs/audit/2026-09-09-final-recheck/native-repro.mjs
node docs/audit/2026-09-09-final-recheck/process-repro.mjs
pwsh -NoProfile -File docs/audit/2026-09-09-final-recheck/snapshot-probe.ps1
```

Rust 脚本在系统临时目录建立隔离工程并离线编译；需要依赖缓存。输出是现状观察，case 名称部分沿用旧版缺陷名，不是产品测试断言。最终应将正确行为写入正式测试套件。
