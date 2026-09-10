# 2026-09-10 审计修复复核

接续昨天任务“全面审计项目代码”中因额度耗尽而中断的修复验证。基线提交为 `14de20b42c06d1d00ce719a4e64d667525413b9f`，被审查对象为当前未提交的修改。对照 [昨天的 13 项审计发现](./项目全面审计报告-2026-09-09.md)，本轮结论是：**部分修复成功，尚未全部修好，并且 IPv6 修复引入了会阻断应用与回滚的系统接口错误。**

本轮只新增报告和隔离复现材料，没有修改业务源码、依赖、锁文件或发布文件。没有实际写入本机 IP、DNS、路由、IPv6 或 DoH；真实 Windows binding 命令只使用本地构造的 CIM 对象和 `-WhatIf`，在参数绑定阶段失败。模拟事务结果不能替代实机验收。

## 逐项状态

“原反例已修复”只针对该条已复验的行为，不代表整个网络修改流程已通过系统集成验收。

| 原编号 | 本轮状态 | 已确认的修复及剩余问题 |
|---|---|---|
| N-01 全局锁超时绕过 | 原超时反例已修复；权限分支仍需收口 | Global 被持有时，第二线程等待 150 ms 后返回失败，不再拿 Local 锁。`AccessDenied` 仍回退 Local；不同权限进程可能使用不同锁，尚不能证明机器级互斥完整。 |
| N-02 全局 DoH 事务 | 部分修复，P1 遗留 | 已保存全部 DoH 表并生成新 DNS 的删除动作，off 恢复也保留模板和 fallback。但删除错误被静默忽略，恢复校验不比较 `doh_settings`；残留全局条目仍可被判完全恢复，见下文。界面仍缺少系统级影响说明。 |
| N-03 加密策略读回 | 原反例已修复 | 模板错误、仅 fallback 错误、要求 off 而实际 manual，三个隔离事务均返回 `success=false`。不能再沿用昨天“仍返回成功”的结论。 |
| N-04 复杂/空配置恢复 | 部分修复，P1 遗留 | 缺少辅助地址/网关、多余 DNS 已加入严格校验；非 DHCP 且无 IP 的原状态会在写入前拒绝。路由 metric、PolicyStore、地址 SkipAsSource 仍未建模，辅助网关 metric 按 `idx + 2` 重建；空静态 DNS 仍按 DHCP 恢复。不能据此承诺复杂配置完整恢复。 |
| N-05 查询失败伪装默认值 | 未修复，P1 | IPv6 读取抛错仍返回 true；DoH 读取抛错仍返回空表。两种独立错误本轮均重现。 |
| N-06 IPv6 目标绑定 | 修改引入新阻断，P1 | 名称查询已改为精确过滤，但应用把 `MSFT_NetAdapter` 对象传给要求 binding 对象的 InputObject，且组合了不兼容参数集，见下文。提交仍无预期 GUID/版本；写入及补偿仍以名称为目标。 |
| N-07 DHCP 被改静态 | 原模式反例已修复 | 前端提交显式 `ipMode=dhcp/dnsMode=dhcp`；后端隔离验证 DHCP 网卡仅处理 IPv6 时只生成一次扩展设置调用，不生成 netsh IP/DNS 写入。DNS 预设会明确选择静态 DNS。实际应用仍被 N-06 阻断。 |
| N-08 恢复告警/现场快照 | 部分修复，P1 遗留 | 无论 rolledBack 是否为 true，都显示 rollbackMessage；有现场快照时更新。`snapshot=null` 时仍保留旧概览，未标记现场状态未知。 |
| N-09 不支持 DoH 的 off 路径 | 原 off 分支已修复；前置检测不完整 | 能力缺失时 off/remove 会跳过。在隔离 binding 错误后，此分支通过；但启用 DoH 的能力检查仍晚于 IP/DNS/IPv6 写入，应该在事务写入前完成。 |
| N-10 旧历史扩大变更意图 | 未修复，P2 | 未带扩展字段的历史仍补 `ipv6Enabled=true`、DoH off；本轮从 IPv6=false 状态载入旧历史后表单变为 true。 |
| N-11 历史跨网卡旧快照 | 未修复，P2 | 载入 B 的历史后选择和表单为 B，快照仍为 A。普通缓存切换和已删除历史目标拒绝的旧修复仍有效。 |
| N-12 Job 失败放行 | 未修复，P1（条件性） | Job 创建可为 None，绑定 bool 仍忽略；普通创建后才绑定，终止返回值仍忽略。本轮普通无害后代进程超时反例通过，不能写成后代存活已复现。 |
| N-13 依赖整改 | 未修改 | package.json、yarn.lock、Cargo.toml、Cargo.lock 与昨天审计指纹完全一致。昨天报告的依赖整改未实施。本轮未重新在线扫描，不把昨天的公告数量当作今天的新扫描结果。 |

## 必须优先处理的阻断

### 1. IPv6 的两个参数错误会同时影响应用和回滚

位置：`src-tauri/src/platform.rs:480–487`；回滚调用在 `:1192` 附近。

当前代码先用 `Get-NetAdapter` 得到适配器对象，然后调用：

```powershell
Enable-NetAdapterBinding -InputObject $targetAdapter -ComponentID ms_tcpip6
Disable-NetAdapterBinding -InputObject $targetAdapter -ComponentID ms_tcpip6
```

本机实际命令元数据要求 InputObject 的类型为 `CimInstance#MSFT_NetAdapterBindingSettingData[]`，不是 `MSFT_NetAdapter`。用本地构造的适配器 CIM 对象及 `-WhatIf` 调用，Enable/Disable 均返回 `MismatchedPSTypeName`。换成正确 binding 类型的本地对象，但保留 ComponentID，两者均返回 `AmbiguousParameterSet`。这是两个独立问题。

[微软 Enable-NetAdapterBinding 文档](https://learn.microsoft.com/en-us/powershell/module/netadapter/enable-netadapterbinding?view=windowsserver2025-ps) 的 InputObject 参数集也不包含 ComponentID。

前端每次应用都会传 IPv6 布尔值；后端会无条件调用相应 Enable/Disable，即使值没有变化。IP/DNS 写入在该调用之前，随后触发的回滚又传 `Some(snapshot.ipv6_enabled)`，因此普通网络修改也会遇到错误，回滚同样无法完整结束。**此次测试确认的是参数绑定失败和调用顺序，没有实际制造本机断网。**

修复方向：先精确选择目标 `Get-NetAdapterBinding -ComponentID ms_tcpip6` 返回的 binding 对象，再仅以 InputObject 调用；完整核验对象归属，值未改变则跳过。使用真实命令参数绑定验证并在可恢复虚拟机验证应用及补偿流程。

### 2. 全局 DoH 残留仍会被误报为已恢复

位置：`src-tauri/src/platform.rs:515–518、1303–1309`，`src-tauri/src/domain.rs:289` 起的恢复校验。

当前快照与补偿 payload 的范围已经扩大，这是有效修复；本轮能看到新 DNS `198.51.100.53` 的 remove 动作。但还存在两个缺口：

- 删除使用 `-ErrorAction SilentlyContinue`。固定生产脚本的删除替身发出非终止错误，脚本仍返回成功。
- `verify_snapshot_restored` 只比较当前网卡的 doh1/doh2，没有核验新服务器所在的系统 DoH 表。隔离事务模拟原网卡 IP/DNS 恢复、但新服务器全局条目仍存在，实际事务结果仍为 `rolledBack=true`，并声称完全恢复。

这说明“发出删除动作”还没有形成“确认删除成功”的闭环。修复时应严格区分条目不存在与读取/删除失败，并对本次实际涉及的服务器比较存在性、template、autoUpgrade、allowFallback；不能只核验恢复后网卡当前使用的两个 DNS。

### 3. 查询失败与旧快照仍被当作当前状态

位置：`platform.rs:423–448、777–793`，`useNetworkConfig.ts:353–356`。

本轮独立注入 IPv6 和 DoH 查询异常，仍分别获得 true 与空表。新的全局 DoH 删除补偿依赖这个空表判断“原先不存在”，因此 N-05 也会破坏 N-02 的恢复依据。应把 unsupported、unknown、已知状态分开，无法可靠读取准备修改的状态时在写入前停止。

前端已经显示恢复诊断，但后端无法读回现场并返回 `snapshot=null` 时，`if (result.snapshot)` 不会清空旧快照。本轮可见状态提示为 RECOVERY_UNKNOWN，概览却继续显示旧 A 快照。应明确将当前概览设为未知；修改前快照可另作人工恢复参考，不能继续充当当前状态。

### 4. 历史载入的两项旧问题尚未处理

位置：`useNetworkConfig.ts:155–160、398–428`。

缺失的 IPv6/DoH 历史字段应保持未指定，不应自动转换为 true/off。跨网卡历史载入还需要取消旧请求、清空不匹配现场快照并重新查询目标状态，同时保留用户载入的历史表单值。当前 watcher 的提前 return 和 fillFromHistory 未查询的组合仍会留下 A/B 混用。

## 验证结果与边界

| 检查 | 本轮结果 |
|---|---|
| `vue-tsc --noEmit` | 通过 |
| 使用当前 Vite 配置的生产构建 | 通过，17 modules；仅禁用 visualizer，输出到本轮证据目录 |
| `cargo test --locked --offline` | 12 passed，0 failed |
| `cargo clippy --locked --offline --all-targets -- -D warnings` | 通过 |
| `cargo fmt -- --check` | 通过；昨天的格式问题已消除 |
| `git diff --check` | 通过 |
| 真实独立审计命名锁 | 第二线程未在 Global 持有期间获取锁，约 150 ms 返回 |
| 真实进程 runner + 无害计时后代 | 500 ms 预算，523 ms 返回，后代完成标记未生成 |
| 真实 Windows binding 参数检查 | Enable/Disable 均证实类型及参数集错误，未执行修改 |
| Rust 事务、Vue composable、固定 PowerShell 脚本隔离复现 | 结果见 [证据说明](./audit/2026-09-10-recheck/README.md) |

没有完成原生 WebView 全流程、真实网络修改、跨权限/登录会话、崩溃恢复、release/MSI 重建或发布验收；本轮也没有重跑依赖在线扫描。模拟器替换的是进程与快照 IO，所以 Rust 事务探针中的“应用成功”不能用来证明 PowerShell 调用有效，N-06 正是这层边界暴露的问题。

建议先修 N-06 的确定性阻断，再完成 N-02/N-05 的恢复与读取闭环，然后处理 N-08/N-10/N-11；复杂配置恢复、Job 失败路径与依赖整改仍应继续跟进。当前不应把这轮修改标记为“13 项全部完成”。
