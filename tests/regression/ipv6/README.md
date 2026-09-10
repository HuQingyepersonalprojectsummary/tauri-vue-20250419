# IPv6 & 网络配置回归测试套件

更新日期：2026-09-10  
所属模块：`tests/regression/ipv6`

---

## 1. 概述与设计原则

本回归测试套件用于在**不修改任何主机物理网络**的前提下，对生产事务逻辑、状态机核验、逆向补偿流程以及前端 Composable 进行高保真隔离测试与故障注入测试。

### 核心设计原则：
1. **零实机网络副作用**：不执行真实系统网络配置更改，不触发真实网卡断网或 IP 变异；
2. **运行生产代码逻辑**：
   - 提取生产环境的 `platform.rs` 与 `domain.rs`，仅把底层进程执行 `run_command_with_timeout` 和 `get_adapter_snapshot` 替换为受控故障注入 Mock；
   - 前端测试直接导入实际运行的 `useNetworkConfig.ts` 与 `useConfigHistory.ts` Composable；
3. **严格失败阻断**：任何一个测试断言失败均以非零状态码退出，阻断 CI/CD 或版本发布。

---

## 2. 目录结构与测试组件

| 文件 | 职责说明 |
|---|---|
| `run.mjs` | 回归测试主入口，按顺序执行各子测试模块并汇总输出 |
| `native-repro.mjs` | 原生有状态事务探针：在临时目录构建轻量 Rust 测试工具，验证 12 个生产级事务与故障恢复场景 |
| `native-edge-repro.mjs` | 原生边界条件与异常状态注入测试（超时、清理拒绝、残留检测等） |
| `frontend-repro.mjs` | 前端状态机测试：模拟 Vue 响应式数据流、网卡切换草稿、历史记录存取与过滤 |
| `powershell-repro.ps1` | PowerShell 脚本生成与参数验证探针，防止参数绑定歧义与拼写错误 |
| `verify-results.mjs` | 验证各子模块输出结果与诊断信息 |
| `*.rs` (fixtures) | 存放受控网络快照数据（静态、DHCP、SLAAC、DoH 配置）与故障模拟桩代码 |

---

## 3. 覆盖的有状态测试场景 (12 Cases)

回归套件覆盖了多次重构和审计发现的经典缺陷与边界情况：

1. **同 IP 前缀替换**：静态 IPv6 仅前缀由 `/64` 改为 `/128` 时的变更与失败补偿；
2. **同前缀来源替换**：原手动地址被同前缀地址替换时的清理与来源核验；
3. **动态 SLAAC/DHCP 地址变化**：原接口包含自动分配地址时，补偿不将自动地址伪造为手动添加；
4. **混合地址模式**：同一网卡同时存在静态地址与自动链路本地/公网地址的处理；
5. **清理命令拒绝**：补偿清理地址时系统命令返回错误，断言 `rolledBack=false`；
6. **静默残留检测**：写入新地址后系统未彻底清除旧地址，断言读回核验失败；
7. **已生效后超时**：命令虽超时但底层地址已添加生效，逆向补偿仍能识别并清理；
8. **空 DNS 输入阻断**：手动 IPv6 DNS 模式下空值或纯空白在写入前被安全拒绝；
9. **未知来源地址保护**：对无法精确识别来源的地址变更在写入前停止，避免破坏后无法恢复；
10. **DoH 全局条目清理核验**：新增 DoH 条目失败时，确认补偿会清除全局条目且核验系统表；
11. **跨网卡历史载入**：载入不同网卡历史时清空过期快照并重新对准目标网卡；
12. **请求序号时序防护**：异步快照读取延迟到达时不覆盖当前最新选中的网卡草稿。

---

## 4. 执行测试

在仓库根目录下运行：

```powershell
npm run test:regression
```

或者直接使用 Node.js 调用：

```powershell
node tests/regression/ipv6/run.mjs
```

测试通过后将输出：
```text
Running native-repro.mjs...
Running native-edge-repro.mjs...
Running frontend-repro.mjs...
Running powershell-repro.ps1...
Passed: 12 stateful transaction cases, prior IPv6 regressions and DNS validation.
```
