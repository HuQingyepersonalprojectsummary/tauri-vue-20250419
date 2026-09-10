# Windows 网络配置工具开发文档

更新日期：2026-09-10。适用于当前 0.1.0 源码和 Windows x64 构建。英文版见 [Developer guide](./WindowsNetworkConfigTool_DevDoc_EN.md)。

## 1. 环境与依赖

本次验证环境为 Node.js 24.16.0、Rust/Cargo 1.91.1、PowerShell 7、Windows MSVC 工具链。需要 Visual Studio C++ 构建工具和 Windows SDK；运行桌面界面需要 WebView2 Runtime。构建环境版本是本次实测值，不代表已验证所有较早版本。

锁定的主要前端依赖：Vue 3.5.13、Vite 6.4.3、TypeScript 5.8.3、Tauri API 1.6.0、Tauri CLI 1.6.3。Rust 依赖精确版本以 Cargo.lock 为准。不要将 package.json 的范围下限当作实际安装版本。

```powershell
npx --yes yarn@1.22.22 install --frozen-lockfile
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
npm run tauri -- dev
```

项目只维护 yarn.lock。npm 可用于执行 scripts，不应使用 npm install 生成第二套锁文件。离线检查要求已经缓存 Cargo 依赖。前端开发服务器使用 3000 端口，端口被占用时会失败而非自动切换。

## 2. 模块分工

| 路径 | 职责 |
|---|---|
| src/App.vue | 网卡列表、表单、DNS 预设、操作反馈与历史界面 |
| src/composables/useNetworkConfig.ts | 当前快照、网卡草稿、请求序号、校验与应用流程 |
| src/composables/useConfigHistory.ts | 版本化历史、坏项过滤、最多 10 条记录、存储告警 |
| src/services/networkClient.ts | 唯一的类型化 Tauri invoke 包装 |
| src/types/network.ts | 前端 DTO 定义 |
| src/utils/validation.ts | IPv4、掩码、网关和 IPv6 输入校验 |
| src-tauri/src/lib.rs | IPC 注册、异步阻塞任务、进程内锁和 Windows 命名互斥体 |
| src-tauri/src/domain.rs | Rust DTO、网络语义校验、快照及地址来源恢复核验 |
| src-tauri/src/platform.rs | 系统工具定位、进程执行、PowerShell 查询、netsh 修改、补偿 |
| src-tauri/src/main.rs | 桌面入口；release 使用 Windows GUI 子系统 |
| src-tauri/tauri.conf.json | 窗口、安全策略、产品名、版本和安装包设置 |
| tests/regression/ipv6 | 生产事务与 composable 的隔离故障注入回归 |
| scripts | 构建与产物导出脚本 |
| releases | EXE/MSI/NSIS、SHA-256 与源码指纹 |

保持领域校验不依赖系统 IO；新的系统操作放入 platform 层；前端通过 networkClient 调用，避免散落 invoke。网络操作必须保留失败原因，并区分应用失败、恢复失败和存储失败。

## 3. IPC 与数据契约

Rust 使用 serde camelCase 与前端交互。DTO 目前由 Rust 和 TypeScript 手工同步；修改字段时同步类型、调用方、快照解析和回归夹具。

| 命令 | invoke 参数 | 返回值 |
|---|---|---|
| get_network_adapters | 无 | AdapterInfo[] |
| get_current_config | `{ adapterName: string }` | AdapterSnapshot |
| apply_adapter_ipv4_config | `{ cfg: Ipv4Config }` | OperationResult |
| greet | `{ name: string }` | 模板遗留问候字符串；不属于网络业务 API |

`apply_adapter_ipv4_config` 为兼容保留的命令名，实际支持 IPv4、IPv6、DNS 与 DoH。

### 变更意图

| 字段 | 语义 |
|---|---|
| adapter | 目标网卡名称；当前仍以名称提交和执行 |
| ipMode / dnsMode | keep 保持、dhcp 自动、static 手动；缺失时兼容旧版字段推断，新调用应显式传入 |
| ip / mask / gateway | 手动 IPv4 地址及掩码必填；网关可选，填写时校验同子网 |
| dns1 / dns2 | IPv4 DNS；仅填 DNS2 会被拒绝；空静态 IPv4 DNS 的恢复语义仍有边界限制 |
| doh1 / doh2 | 对应 IPv4 DNS 的 off/auto/manual、template、allowFallback |
| ipv6Enabled | true/false 修改绑定；缺失表示保持绑定状态 |
| ipv6Mode / ipv6DnsMode | keep/dhcp/static；缺失按 keep 处理，不用旧快照猜测用户意图 |
| ipv6Ip / ipv6Prefix / ipv6Gateway | 静态 IPv6 地址、1–128 前缀和可选网关；后端未传前缀时默认为 64 |
| ipv6Dns1 / ipv6Dns2 | 手动 DNS1 必填、DNS2 可选；空值及纯空白在任何写入前拒绝 |

仅修改 IPv6 DNS 的例子：

```typescript
await networkClient.applyAdapterIpv4Config({
  adapter: '以太网',
  ipMode: 'keep', dnsMode: 'keep',
  ip: '', mask: '', gateway: '', dns1: '', dns2: '',
  ipv6Mode: 'keep', ipv6DnsMode: 'static',
  ipv6Dns1: '2001:db8::53', ipv6Dns2: ''
});
```

示例地址为文档地址，实际使用时替换为所需服务器。不要为了补全表单而把 undefined 改成 dhcp/static；旧历史和网卡草稿往返必须保持相同意图。

### 快照与结果

AdapterSnapshot 包含接口名称、GUID/index、状态、IPv4 地址列表、网关、DNS、DoH 状态和 IPv6 详细字段。IPv6 地址保存 ipAddress、prefixLength、prefixOrigin、suffixOrigin；来源缺失保持未知。来源信息用于区分原手动地址与 DHCP/SLAAC 地址，不能仅按 IP 字符串判断恢复完成。

当目标适配器处于未绑定或禁用 IPv6 协议栈状态（如系统组件 `ms_tcpip6` 为 False）时，底层 CIM 查询 `Get-NetIPInterface -AddressFamily IPv6` 与 `Get-DnsClientServerAddress` 会抛出“找不到匹配对象”异常。快照脚本捕获该空匹配异常并执行安全回退（IPv6 地址/DNS/网关为空列表，`ipv6Enabled=false`，`ipv6DhcpEnabled=true`），确保不影响 IPv4 配置读取。

OperationResult 的含义：

- success：请求通过了应用后的读回核验。
- message：应用结果或原始失败原因。
- rolledBack：补偿命令成功且恢复读回核验通过；不等于“尝试过回滚”。
- rollbackMessage：恢复诊断；失败时必须向用户显示。
- snapshot：最终读到的现场；无法读回可为 null，前端应清空旧概览，不能继续展示为当前状态。

参数或前置能力检查失败可直接 reject IPC Promise；已经开始修改后的失败通常返回 success=false 及补偿诊断。不能把两种失败路径当作同一类返回。

## 4. 事务执行与恢复

1. IPC 将阻塞系统工作放到 spawn_blocking，写操作持有进程内锁和 Windows 命名互斥体。
2. 校验 IPv4/IPv6、DNS 与 DoH；获取原现场；检查状态、DoH 能力及需要修改的 IPv6 地址来源。
3. 按明确模式执行 IPv4 地址/DNS、扩展设置、IPv6 地址和 DNS。keep 跳过该项主动写入。
4. 每次尝试删除/添加 IPv6 地址前记录 IP，和不可变的原快照一起保存原前缀及来源；命令超时也按可能已经生效处理。
5. 读回并比较请求与实际状态，IPv6 地址比较采用规范化语义，DNS 比较完整数量、顺序和值。
6. 出错后尝试补偿。清理本次触及地址，包括同 IP 的前缀/来源替换；原手动地址按前缀恢复，原自动地址由自动机制重新获取。
7. 再次读回恢复现场。静态残留、未知来源、未恢复的手动前缀或命令失败，都不能返回 rolledBack=true。自动地址可重新分配，不要求 DHCP/SLAAC 地址集合恒定。

该流程不是操作系统原子事务。它没有持久化事务日志，进程崩溃/重启后无法继续内存中的补偿。复杂路由属性、多网关/多 DNS、地址生命周期等恢复范围仍见 [已知限制](./docs/known-limitations.md)。

## 5. 系统执行与权限

PowerShell 使用固定脚本，通过 stdin JSON 传入数据；netsh 使用独立参数数组传递。系统工具由可信系统目录定位，不能将用户输入拼进脚本。子进程配置 CREATE_NO_WINDOW；release 主程序使用 GUI 子系统。

针对底层调用的加固与容错机制：
1. **Netsh 参数引用安全与错误回退**：底层 netsh 参数独立分词，对含空格的网卡名称做安全转义；同时针对部分 Windows 语言环境下 netsh 失败信息输出至标准输出的现象，`format_process_error` 在 stderr 为空时自动回退读取 stdout，防止错误诊断信息丢失。
2. **管理员权限前置检查 (`is_elevated`)**：写操作前主动校验 Windows Access Token 管理员安全组凭据。若在普通用户权限下运行，直接返回明确的提权指引，避免逐条命令失败触发不必要且冗长的逆向补偿。
3. **CIM 查询多语言容错**：快照采集与 DoH 配置中的 CIM 查询（`Get-NetIPInterface`、`Get-DnsClientServerAddress`、`Get-DnsClientDohServerAddress`）统一配置 `-ErrorAction Stop` 并结合正则捕获多语言空对象异常，保证在未绑定 IPv6 或系统无 DoH 映射时平滑回退，不阻断常规网卡信息读取。

执行器设置超时并使用 Windows Job Object 管理子进程，但 Job 创建/绑定失败及终止确认仍有待完善的路径。命名互斥体在 Global 等待超时后不会改取 Local；权限拒绝回退 Local 的跨权限隔离仍未完成验证。

当前没有按需提权 helper，也没有为主应用配置 requireAdministrator 清单。管理员运行由使用者选择；不能把安装器 UAC 当作主程序自动提权。系统级 DoH 条目可能被多网卡共用，变更时应理解其影响范围。

## 6. 前端状态与历史

快照与表单分开维护。异步读取用请求序号忽略过期结果，切换网卡恢复对应内存草稿。载入历史后重新查询目标状态，同时保留历史表单值；不存在的历史网卡会被拒绝。

历史存储键为 net_config_history_v1，schemaVersion 为 1，上限 10 条。读取时逐项校验和过滤，保存异常独立提示。记录不是系统快照备份，也不代替人工恢复方案；草稿不是跨进程持久化数据。

## 7. 检查、构建与维护

```powershell
npm run typecheck
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings
npm run test:regression
npm run release
```

- **单元测试 (17 项)**：涵盖 IPv4/IPv6 格式校验、子网连续掩码换算、网关同网段断言、DNS/DoH 组合校验、快照读回校验、跨进程互斥锁超时、子进程静默执行、Netsh 参数格式化、进程错误输出回退以及管理员特权检查。
- **回归测试 (12 项)**：测试当前源码，临时 Rust 工程仅替换系统 IO，Vue 使用实际 composable。测试输出进入被忽略的 tests/regression/ipv6/output；任何探针失败会让命令非零退出。修改生产函数边界时必须同步提取探针并验证它仍运行生产逻辑。

构建命令生成 EXE、MSI 和 NSIS，并将本轮文件导出到 releases。scripts/export-release.ps1 在复制前检查完整产物集合及构建时间，生成源码指纹和 SHA-256。详见 [打包说明](./打包说明.md)。

维护版本时同步 package.json、src-tauri/Cargo.toml、Cargo.lock 中本项目版本及 tauri.conf.json；更新依赖时同时更新对应锁文件并重跑检查。提交前检查源码、文档、产物哈希和 diff，推送使用普通 fast-forward，避免覆盖远端工作。

审计过程的重复日期报告和原始日志已收敛为 [验证记录](./docs/verification.md) 与 [已知限制](./docs/known-limitations.md)，可复用的探针迁入 tests。不能将历史发现的删除理解为所有缺陷已关闭。
