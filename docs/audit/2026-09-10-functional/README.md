# 功能逻辑复审：开关、状态同步与事务恢复

审计日期：2026-09-10。代码基线：`7f2497d32a6c9d1a9995e59e7ba0ce9b54ddf1f6`。

结论：发现 **8 项可定位的功能问题（4 项 P1、4 项 P2）**。既有测试通过，但不足以证明当前版本的开关、DoH 清理与恢复逻辑正确。本轮增加审计探针和证据，未修改业务实现、发布包或本机网络配置。

P1 表示应优先修复的错误写入或恢复缺陷；P2 表示特定输入、界面路径或状态更新异常。以下“复现”均指生产代码的隔离执行；不等同于 Windows 实机网络变更验收。

## 发现汇总

| 编号 | 级别 | 问题 | 验证方式 |
|---|---|---|---|
| FA-01 | P1 | 保持 DNS 或未指定 DoH，仍会清理旧 DNS 的 DoH | 真实 Rust 事务 + 有状态 IO 模拟 |
| FA-02 | P1 | 单网卡 DoH 回滚错误地采用全局模板和策略 | 真实 Rust 补偿函数生成的操作载荷 |
| FA-03 | P1 | IPv6 DNS 的 DoH 清理未纳入完整补偿 | 故障注入，确认恢复失败且未生成对应恢复操作 |
| FA-04 | P1 | 等待权限检查时可改变最终提交的网卡 | 真实 Vue composable + 延迟 IPC |
| FA-05 | P2 | 关闭 IPv6 被隐藏的静态字段校验阻断 | 前端提交载荷 + 后端校验复现 |
| FA-06 | P2 | IPv4 IP/DNS 界面缺少“保持”，改 IPv6 会重提静态配置 | Vue 载荷复现 + 模板与写入分支检查 |
| FA-07 | P2 | 空的手动 IPv4 DNS 可显示成功，实际仍为 DHCP | 真实 Rust 事务 + 快照读回 |
| FA-08 | P2 | 网卡列表变空后，仍展示旧网卡快照和表单 | 真实 Vue composable |

## FA-01：保留意图会触发 DoH 关闭

位置：[platform.rs:2327](../../../src-tauri/src/platform.rs#L2327)，关联 `2300–2324` 的 `configured_doh_ips` 构造与 `2713–2800` 的核验分支。

`configured_doh_ips` 仅登记 `dns_mode == "static"` 且显式传入 `doh1/doh2` 的服务器。但旧 DNS 遍历清理不检查 `dns_mode`，凡未登记的旧服务器都被追加 `mode=off`。因此 `dnsMode=keep`、未指定 DNS/DoH 的旧版载荷、以及手动 DNS 留空均可能清理现有 DoH。对于历史载入中缺少 DoH 字段的配置，该路径也会发生。

探针 `keep_dns_turns_doh_off`：原快照为加密 DNS，提交保持模式后，唯一写操作包含原 DNS 的 `off`，模拟系统状态随此操作改变，最终返回 `success=true`、`snapshot.doh1.mode=off`。这不是仅界面未刷新，而是生成了关闭命令且核验未阻止。

影响：改变用户没有请求修改的 DNS 加密策略。PowerShell 的关闭分支还修改全局 `AutoUpgrade`，不能只将此理解为单网卡显示变化。

修复方向：明确区分“保持”“显式关闭”“替换 DNS 后清理”；仅在用户明确改变该 DNS 项时生成清理，并核验未修改项的不变性。全局与单网卡策略分别处理。

## FA-02：回滚用全局 DoH 重建单网卡设置

位置：[platform.rs:1651](../../../src-tauri/src/platform.rs#L1651)，关联 `1130–1190` 的单网卡读回和 `752–865` 的注册表写入。

最新实现读回时以单网卡 `adapterDohSettings` 为优先来源，但补偿仍只读取 `snapshot.doh_settings`（全局表），以“全局模板非空”推断 `manual`，再调用同一个同时修改全局表和单网卡注册表的写函数。

探针 `rollback_uses_global_not_adapter_doh`：修改前网卡为 `auto`、禁止回退、使用网卡模板；全局表有不同模板且允许回退。补偿生成的是 `manual`、全局模板、允许回退。即使模板相同，自动模式也会因全局表模板非空被改为手动。

影响：发生其他网络配置错误时，DoH 原设置不能正确恢复。当前恢复核验可能正确报 `rolledBack=false`，但它只能发现问题，无法补回遗漏的信息；本报告不将此描述为所有场景都会假报恢复成功。

修复方向：快照保存完整单网卡 DoH 条目、是否原本存在、模式/模板/回退与原始恢复所需字段；全局表和单网卡条目分开恢复，不能从全局表反推单网卡状态。

## FA-03：IPv6 DNS 清理没有对应的 DoH 回滚

位置：[platform.rs:2339](../../../src-tauri/src/platform.rs#L2339)，关联 `2105–2119` 的 `touched_dns`、`1639–1648` 的恢复集合。

IPv6 DNS 切换至 DHCP 会给所有旧 IPv6 DNS 添加 DoH `off` 操作。但 `touched_dns` 只登记 IPv4 DNS，恢复集合也只追加 `snapshot.dns_servers`，未包括被清理的 `ipv6_dns_servers`。完整单网卡 IPv6 DoH 映射也未保存在 Rust 快照中。

探针 `failed_ipv6_dns_does_not_restore_doh`：在清理原 IPv6 DNS `2001:db8::53` 的 DoH 后，让 IPv6 DNS 的 DHCP 命令失败。生产补偿流程继续执行，但该 IPv6 DNS 在所有 DoH 操作中只出现一次 `off`，没有恢复；最后返回 `rolledBack=false`，诊断全局 DoH 状态不一致。

影响：一次失败的 IPv6 DNS 切换可以留下 DoH 被关闭的现场，自动补偿无法完整恢复。

修复方向：在每项可能生效的 DoH 写入前登记实际服务器，不限地址族；补偿所需全局与单网卡旧值一并记录，并对所有触及项核验。

## FA-04：权限检查期间没有冻结提交意图

位置：[useNetworkConfig.ts:462](../../../src/composables/useNetworkConfig.ts#L462)，关联 `477–507`。

`applyConfig()` 先校验当前表单，然后 `await checkAdminPrivilege()`；`isLoading=true` 和载荷构造在等待结束之后。等待期间下拉框、历史载入、开关和提交按钮仍可操作，最终载荷直接读取最新 `selectedAdapter` 和 `ipConfig`。

探针 `admin_await_changes_apply_target`：在 A 上点击应用，延迟权限响应，切换到 B，再返回有权限。确认等待时 `isLoading=false`，实际 IPC 提交 `adapter=B`。修改其他字段也可能绕过此前针对旧字段的前端校验；后端虽会重新校验，但无法识别用户原本想提交 A。

影响：点击时的目标与实际写入目标不同；连点也可能排入重复事务。后端写锁只能串行执行，不能恢复前端丢失的原始意图。

修复方向：函数入口做忙碌防护，第一次等待前冻结目标和完整载荷，统一设置忙碌状态；校验、提权检查、写入均针对同一份载荷。

## FA-05：关掉 IPv6 后仍校验隐藏字段

位置：[platform.rs:2039](../../../src-tauri/src/platform.rs#L2039)，关联前端 `useNetworkConfig.ts:409–459`、`App.vue:515–527`。

触发：启用 IPv6 时将地址或 DNS 改成手动，留空或填错，再关闭 IPv6，点击应用。前端隐藏详细字段并跳过 IPv6 校验，但仍提交原 `ipv6Mode` / `ipv6DnsMode` 与无效字段；后端的静态校验没有 `ipv6_enabled != Some(false)` 条件。

前端探针确认 `ipv6Enabled=false`、`ipv6Mode=static`、`ipv6Ip=""` 确实提交；两个原生探针分别返回“IPv6 地址不能为空”和“手动 IPv6 DNS 必须填写首选 DNS”，写调用数为 0。

影响：用户看到开关已关，应用却因不可见字段失败。这是本轮与“开关不正常”最直接相关的复现之一。有效静态字段下关闭 IPv6 不受这一特定问题影响。

修复方向：统一前后端有效字段规则；显式关闭绑定时不校验、不执行 IPv6 地址/DNS编辑字段，并覆盖“编辑无效字段→关闭→应用”的交互回归。

## FA-06：IPv4 未提供保持现状入口

位置：[App.vue:280](../../../src/App.vue#L280)、[App.vue:342](../../../src/App.vue#L342)，关联 `useNetworkConfig.ts:204–205`、`platform.rs:2189`。

IPv4 IP 和 DNS 下拉框只有 DHCP/static，虽然类型和后端支持 `keep`，README 也声明四类配置可独立保持。快照一旦读为静态，修改其他开关也会再次提交这些静态字段；后端 static 分支无差异判断，仍执行静态 IP/DNS 写入。

探针 `ipv6_toggle_resubmits_unchanged_static_ipv4_dns` 确认只改变 IPv6 开关时，IPv4 IP/DNS 仍作为 static 提交。静态配置读回后若被其他程序改变，再只操作 IPv6，应用会重写表单保存的旧 IPv4/DNS。当前 UI 没有明确表达“这两项保持现在系统状态”的入口。

修复方向：补齐保持选项及历史展示；将当前状态与本次编辑意图区分。静态多地址/路由变更风险仍须结合既有已知限制处理，本探针没有模拟 Windows 的多地址删除行为。

## FA-07：空手动 DNS 成功结果与模式不符

位置：[platform.rs:2241](../../../src-tauri/src/platform.rs#L2241)、[platform.rs:2713](../../../src-tauri/src/platform.rs#L2713)。

触发：当前为 DHCP DNS，切换手动 IPv4 DNS，首选/备用均空、DoH 关闭，应用。前后端允许这组输入；写入分支因 `dns1_opt=None` 不配置 DNS；核验也仅在至少一个 DNS 非空时检查 DHCP 关闭。

探针 `static_dns_blank_keeps_previous` 返回 `success=true`，但 `snapshot.dnsDhcpEnabled=true`，原 DNS 仍在。表单手动模式与实际 DHCP 状态不一致。其 DoH 清理副作用已单独记为 FA-01。

修复方向：手动模式要求有效首选 DNS，或将空值明确归一为“保持”；不能把未实现的模式切换报告为成功。

## FA-08：无网卡时旧快照未清空

位置：[useNetworkConfig.ts:154](../../../src/composables/useNetworkConfig.ts#L154)，关联 `246–296` 的选择监听器。

读取 A 后刷新列表返回空数组，`selectedAdapter` 置空，但 watcher 仅处理非空新网卡，没有清空分支。探针 `no_adapters_retains_old_snapshot` 确认 selected 为空时，快照和表单仍指向 A。

影响：概览继续显示已消失网卡的 IPv6 开关、DNS 等状态，容易被误认为实时结果；应用按钮已有空选择保护，因此没有将它认定为可直接向空网卡写入。

修复方向：列表为空时使旧请求失效，清空快照、表单及相应加载状态；草稿保留与当前状态展示独立处理。

## 验证与范围

| 检查 | 本轮结果 |
|---|---|
| `npm run typecheck` | 通过 |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline` | 17 passed，0 failed |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 通过 |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings` | 通过，无警告 |
| `npm run test:regression` | 通过：12 个 stateful cases 及既有 IPv6/前端/PowerShell 检查 |
| 本目录 `frontend-probe.mjs` | 生产 Vue 状态逻辑，6 个观察场景 |
| 本目录 `native-probe.mjs` | 生产 Rust 事务/补偿逻辑，6 个观察场景 |

探针的断言用于**证明缺陷在当前基线存在**；探针进程退出 0 不代表产品无缺陷。证据见 [frontend-results.json](frontend-results.json)、[native-results.json](native-results.json)。修复后应将相应观察转换成期望正确行为的回归断言。

复跑命令：

```powershell
node docs/audit/2026-09-10-functional/frontend-probe.mjs
node docs/audit/2026-09-10-functional/native-probe.mjs
```

审阅覆盖 `App.vue`、两个 composable、IPC 类型与客户端、`lib.rs` 权限/写锁、`domain.rs` 校验/恢复比对、`platform.rs` 快照/写入/补偿、已有回归脚本和文档。图谱可用于发现入口，但索引行号和部分结构落后于当前提交，定位以磁盘源文件为准。

另外两个前端探针展示“同网卡在途读覆盖历史”和“请求换代后 loading 残留”的内部 API 边界。常规 UI 的读回期间按钮禁用会阻止这些直接序列，因此**没有将它们计入上述 8 项用户可触发/功能合同问题**。

未执行真实网卡启停、IP/DNS 写入、注册表变更、抓包验证、安装包安装卸载；未重新构建发布包，也没有做新一轮依赖漏洞扫描。DoH 注册表数值、写入后 DNS 客户端是否真实采纳、不同 Windows 构建版本的行为仍需隔离虚拟机验收。读写同一注册表值不能代替真实加密解析验证。

官方资料交叉核查：Microsoft 的 [DoH 客户端文档](https://learn.microsoft.com/en-us/windows-server/networking/dns/doh-client-support) 区分要求加密、允许回退和明文模式，并说明已知服务器表的作用；[DNS_DOH_SERVER_SETTINGS](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/ns-netioapi-dns_doh_server_settings) 提供公开 API 的模式及回退标志定义。后者的 API 标志不能直接推定为本项目使用的内部注册表 `DohFlags` 编码，本轮未据此宣称项目魔数错误。

建议先修 FA-01–04，再统一开关/模式的有效字段规则并修 FA-05–08。之后运行新增正确性回归，并在可还原的 Windows 虚拟机上完成“启用→关闭→重开→刷新→切换网卡→失败恢复”的完整验收。
