# 2026-09-10 修复复核证据

对应 [修复复核报告](../../audit-recheck-2026-09-10.md)。源码指纹见 `review-manifest.json`。

所有命令在仓库根目录执行；脚本输出观测值，退出码 0 不代表被测业务没有缺陷。

```powershell
node docs/audit/2026-09-10-recheck/frontend-repro.mjs
node docs/audit/2026-09-10-recheck/native-repro.mjs
node docs/audit/2026-09-10-recheck/mutex-repro.mjs
pwsh -NoProfile -File docs/audit/2026-09-10-recheck/powershell-repro.ps1
node docs/audit/2026-09-09-final-recheck/process-repro.mjs
node docs/audit/2026-09-10-recheck/build-check.mjs
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
node node_modules/vue-tsc/bin/vue-tsc.js --noEmit
```

- `native-repro.mjs`、`native-fixtures.rs`：从当前真实源码复制领域及平台函数到临时 Cargo 项目，替换全部进程执行和快照 IO，检查未残留 `.spawn()`。保留真实事务、补偿和恢复判定。继承昨天反例并新增独立 fallback、全局 DoH 残留、DHCP no-op、空静态 IP 写入前拒绝。`residual_global_doh` 的错误读回来自替身，证明控制流遗漏，不代表真实机器已出现该状态。
- `native-results.json`：包含完整调用及 snapshot。原 `empty_static_restore_commands` 直接调用补偿函数，不能用它否定新增事务前置拒绝；事务级拒绝结果应看 `empty_static_preflight`。`dhcp_ipv6_only` 只证明无 netsh 写入，不证明扩展脚本有效。
- `frontend-repro.mjs`：实际 composable、真实 Vue 调度，IPC/storage 为替身。旧 case 名称沿用昨天，是否修复看观测值；新增 `unknown_recovery_retains_old_snapshot`。loading 竞态通过 API 直接触发，UI 禁用控件限制此路径，未作为主要新发现。
- `powershell-repro.ps1`：真实 Windows cmdlet 仅通过命令元数据及本地 CIM 对象的 `-WhatIf` 调用核验类型/参数集，未枚举真实网卡。生产快照/应用脚本通过 stdin JSON 运行，全部系统查询和写入由局部函数替换。能力测试中 IPv6 binding 使用宽松替身，以独立观察 DoH 分支；不能把该成功结果当作真实 IPv6 调用成功。
- `powershell-results.json`：含两个真实命令的参数集，以及两类真实参数绑定失败；另含 IPv6/DoH 读取异常、unsupported off/manual、off 精确恢复及删除错误吞掉的模拟结果。
- `mutex-repro.mjs`：真实 Windows mutex 模块与唯一审计名称，不占用应用的锁。仅验证超时分支，未跨权限或跨登录会话验证。
- `process-results.json`：重用已有无害计时进程探针，提取当前 runner，没有业务网络入口。
- `dependency-comparison.json`：四个依赖配置/锁文件与昨天审计 SHA-256 相同。本轮没有在线重新扫描；昨日公告结果保留在原证据目录。
- `cargo-test.txt`、`clippy.txt`、`fmt.txt`、`typecheck.txt`、`build-output.txt`：当前构建检查日志；成功的 typecheck/fmt 输出为空。构建移除 visualizer，避免覆盖原有 stats.html。

生成的 `build/` 不提交。没有修改业务源码或原审计材料。
