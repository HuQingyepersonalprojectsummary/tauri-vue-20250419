# Windows 网络配置工具验证记录与测试报告

更新日期：2026-09-10  
适用版本：v0.1.0 (Windows x64)

本文档归纳当前 Windows 网络配置工具在开发、重构和多轮审计整改后的自动化验证结果、故障注入测试覆盖率以及推荐的实机集成验收流程。

---

## 1. 自动化验证矩阵

当前代码基线在本地标准 Windows x64 开发构建环境中已通过全部离线静态检查、单元测试、有状态故障注入回归测试及发布构建：

| 检查项 | 验证命令 | 结果 | 覆盖范围与说明 |
|---|---|---|---|
| **Rust 单元测试** | `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline` | **14 passed** / 0 failed | IPv4/IPv6 格式校验、连续掩码计算、网关同子网断言、DNS/DoH 组合校验、跨进程全局锁超时测试、子进程静默执行（CREATE_NO_WINDOW）测试 |
| **Rust 代码规范** | `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings` | **0 warnings** | 严格启用零警告策略（`-D warnings`），涵盖全 target（lib、bin、tests） |
| **Rust 代码格式** | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | **0 diff** | 统一采用 `rustfmt` 标准格式规范 |
| **前端类型检查** | `npm run typecheck` (`vue-tsc --noEmit`) | **0 errors** | 严格类型模式，Vue 组件、Composable、API 客户端及网络 DTO 类型全部匹配 |
| **有状态事务回归** | `npm run test:regression` (`node tests/regression/ipv6/run.mjs`) | **12 passed** / 0 failed | 真实事务逻辑注入受控 IO：覆盖同 IP 替换补偿、来源变异、命令超时、DNS 校验、PowerShell/Vue composable 隔离探针 |
| **全量打包验证** | `npm run release` (`scripts/build-release.ps1`) | **Success** | 成功生成便携 EXE（中英文双命名）、MSI 安装包、NSIS 安装包及对应 SHA256SUMS.txt 和 release-manifest.json |

---

## 2. 核心审计发现整改闭环

在历次代码审计与安全加固中发现的风险点已完成系统性修复与测试闭环：

### 2.1 RR6-01：同 IP 替换的补偿与地址来源精确核验
- **问题背景**：当用户修改静态 IPv6 前缀（如将 `/64` 改为 `/128`）或替换相同 IP 时，旧逻辑可能误判“原先已有该 IP”而跳过清理，导致网卡残留多个同 IP 前缀或来源冲突。
- **修复方案**：
  - 快照增强记录每个 IPv6 地址的 `PrefixOrigin` 与 `SuffixOrigin`。
  - 在每条底层 `netsh` 或 PowerShell 地址命令执行前，登记实际触及的目标 IP，结合执行前不可变的原始快照进行精准逆向补偿。
  - 读回核验严格比对被触及地址的恢复状态，若检测到未知来源或无法恢复的原手动前缀，明确标记 `rolledBack=false` 并输出诊断信息。
- **验证**：通过 `tests/regression/ipv6/` 中 12 项有状态注入场景，包含同 IP 前缀替换、动态 SLAAC 地址混合存在时的恢复隔离。

### 2.2 RR6-02：手动 IPv6 DNS 边界校验统一
- **问题背景**：前端界面与后端 Rust 校验规则在手动 IPv6 DNS 场景下不一致，空输入或只填备用 DNS 会穿透到写入阶段才发生异常。
- **修复方案**：
  - 前端界面与 Rust `domain.rs` 统一要求：手动 IPv6 DNS 模式下，**首选 DNS（DNS1）必须为合法 IPv6 地址**；备用 DNS（DNS2）可选。
  - 阻断纯空白输入与“仅填 DNS2”等非法组合，未通过校验时禁止触发底层系统 IO。
- **验证**：前端 Composable 单元测试与 Rust `test_validate_dns_combination` 均断言通过。

### 2.3 静默后台执行与进程树生命周期
- **问题背景**：调用 `netsh` 或 `powershell` 时可能在用户屏幕闪现黑框命令行窗口；极端超时时可能遗留孤儿进程。
- **修复方案**：
  - 子进程创建统一添加 `CREATE_NO_WINDOW` 标志；
  - 引入 Windows Job Object（作业对象），绑定子进程并配置作业关闭即杀死后代进程；
  - 单元测试 `test_run_command_silent_background_execution` 验证了命令行在后台静默运行。

### 2.4 全局并发互斥锁加固
- **问题背景**：多实例同时操作可能引发底层网卡配置竞争破坏。
- **修复方案**：
  - 引入 Windows 命名全局互斥体 `Global\tauri_network_config_tool_mutex`，配合 150ms 超时快速失败，防止连击和多开竞态；
  - 单元测试 `test_cross_process_lock_timeout_prevents_local_bypass` 验证了锁超时能正确拦截并发请求。

---

## 3. 发布物完整性记录

最新一轮全量打包产物指纹如下（已同步记录至 `releases/SHA256SUMS.txt`）：

| 产物名称 | 大小 (Bytes) | SHA-256 哈希值 |
|---|---|---|
| `Windows网络配置工具.exe` | 2,084,352 | `c2933a7604ed0941976b851732e80d1172606ee2a278165f6dd7761354719335` |
| `Windows_Network_Config_Tool_v0.1.0.exe` | 2,084,352 | `c2933a7604ed0941976b851732e80d1172606ee2a278165f6dd7761354719335` |
| `Windows网络配置工具_0.1.0_x64_zh-CN.msi` | 1,462,272 | `e78485de62872d00debdd52fe235c7333ccaa36df6a6cfa66f116577523c6740` |
| `Windows网络配置工具_0.1.0_x64-setup.exe` | 949,727 | `36354312de03ab351c842f614cdc3a31a11e254897a18f246d4d814abb7d1cb6` |

> [!NOTE]
> 当前发布产物未配置商业 Authenticode 代码签名证书。SHA-256 校验和用于完整性核验，不代表发布者机构身份认证。

---

## 4. 推荐实机环境验收清单

在投入生产环境前，建议在受控的 Windows 虚拟机（具备系统快照还原能力）中执行以下物理验收流程：

1. **UAC 启动行为验收**：
   - 双击直接启动（普通用户权限）：应能正常枚举网卡并显示当前 IP/DNS 状态；点击应用时应优雅捕获并提示管理员权限不足。
   - 右键“以管理员身份运行”：应具备完整的配置修改权限。
2. **常规网络配置验收**：
   - 动态转静态：将目标网卡由 DHCP 切换为手动指定 IPv4/掩码/网关/DNS，验证 `ipconfig /all` 读回生效且外网连通。
   - 静态转动态：将网卡由静态切换回自动获取（DHCP），验证能够重新从路由器租约获取地址。
   - IPv6 手动指定与清除：配置静态 IPv6 及 DNS，验证能够正常解析并在界面显示。
   - DoH 加密配置：在支持 DoH 的 Windows 11 环境上启用 DoH 模板，在 PowerShell 中执行 `Get-DnsClientDohServerAddress` 验证已生效。
3. **故障补偿验收**：
   - 在故意配置冲突网关或模拟底层中断时，验证应用返回 `rolledBack=true`，界面输出恢复诊断，且原有网络状态被原样恢复。
4. **安装包生命周期验收**：
   - 测试 MSI 和 NSIS 安装程序在目标机器上的静默/交互式安装；
   - 验证快捷方式正常生成，应用能正常运行，并在控制面板卸载时干净清除安装目录。
