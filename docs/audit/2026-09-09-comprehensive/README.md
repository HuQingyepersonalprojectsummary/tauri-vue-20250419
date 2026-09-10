# 第四轮审计证据

对应 [全面审计报告](../../项目全面审计报告-2026-09-09.md)，基线 `14de20b42c06d1d00ce719a4e64d667525413b9f`。

以下命令在仓库根目录执行。所有业务修改均使用替身，没有真实网络写入。

```powershell
node docs/audit/2026-09-09-comprehensive/native-repro.mjs
node docs/audit/2026-09-09-comprehensive/frontend-repro.mjs
node docs/audit/2026-09-09-comprehensive/mutex-repro.mjs
pwsh -NoProfile -File docs/audit/2026-09-09-comprehensive/powershell-repro.ps1
node docs/audit/2026-09-09-final-recheck/process-repro.mjs
node docs/audit/2026-09-09-comprehensive/build-check.mjs
pwsh -NoProfile -File docs/audit/2026-09-09-comprehensive/dependency-scan.ps1
```

- `native-repro.mjs`：复制当前 Rust 领域/平台源码到临时 Cargo 项目，在编译前替换进程及快照 IO；保留实际事务、补偿、DoH payload 构造和校验。禁止残留 `.spawn()`。结果是控制流证据，不是系统集成测试。`new_doh_rollback` 模拟 DoH 写入后进程超时，检查是否针对新服务器生成恢复动作。
- `frontend-repro.mjs`：执行实际 TypeScript composable、真实 Vue 调度；IPC 与 storage 为替身。额外 loading 竞态通过直接调用 API 构造，报告没有认定为正常界面必现。
- `mutex-repro.mjs`：提取实际 Windows mutex 模块，用两个线程及唯一审计名称复现跨命名空间回退；不使用应用锁、不执行网络操作。
- `powershell-repro.ps1`：执行源码中固定脚本，全部网络/注册表命令由局部函数替换。仅去掉重置 Console.In 的 encoding setter 以注入 JSON。绑定匹配替身按 PowerShell 通配语义处理；系统 CDXML 元数据另存于 JSON。
- `process-results.json`：重用上一轮无害计时子进程探针，但提取的是当前 runner；本轮后代标记未生成。
- `build-output.txt` 与 `build/`：实际 Vite 配置构建结果；禁用 visualizer 避免修改根目录 stats.html。
- `npm-advisories.json`、`dependency-summary.json`：本轮 Yarn 在线结果精简、GHSA 去重统计。Rust `rust-osv.json` 保留原公告 ID 和别名；不要直接把别名加总为漏洞数。
- `windows-dependency-tree.txt`：锁定 Windows normal/build 依赖树；只能证明依赖路径，不能证明漏洞函数可达。
- `release-artifacts.json`：现有 EXE 只读指纹，未执行或重建发布物。
- `review-manifest.json`：源码与锁文件指纹，供后续复核识别基线。

脚本输出的是观测值，不以“进程退出 0”表示被测业务正确。Rust 编译使用本机缓存；系统不具备 Windows/Rust 工具链时，探针不能直接运行。
