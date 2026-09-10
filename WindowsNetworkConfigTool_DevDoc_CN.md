# 🛠️ Windows 网络配置工具开发文档 (中文)

## 🏁 一、项目简介

本项目基于 [Tauri](https://tauri.app/) + [Vue 3](https://vuejs.org/) 技术栈开发，为 Windows 用户提供安全、直观、可靠的网络适配器配置工具。用户可通过图形界面查看、修改本机网络适配器的：
- **IPv4 地址与掩码**（支持严格连续二进制掩码校验与零前导校验，支持 DHCP 与静态模式独立切换）
- **默认网关**（支持同网段可达性校验）
- **常规 DNS 服务器**（首选与备用 DNS 依赖关系校验）
- **DNS over HTTPS (DoH) 加密解析**（开/关、开(自动)、开(手动模板) 及未加密请求回退）
- **IPv6 协议组件绑定状态**（安全启用或禁用适配器 `ms_tcpip6` 协议）
- **IPv6 地址与前缀分配**（支持 SLAAC / DHCPv6 自动分配与静态手动指定）
- **IPv6 首选与备用 DNS**（支持 DHCPv6 自动获取与静态指定首选/备用权威 DNS）
- **常用公共 DNS / DoH / IPv6 一键预设**（阿里 DNS、腾讯 DNSPod、Cloudflare、Google）
- **带版本校验与安全持久化的配置历史记录**（上限 10 条）
- **全流程静默后台执行**（集成 Windows `CREATE_NO_WINDOW`，彻底消除黑框弹窗闪烁）

---

## 二、架构设计与安全理念

```mermaid
flowchart TD
  UI[Vue 3 前端界面] -->|类型化 IPC| IPC[Tauri Commands lib.rs]
  IPC -->|串行写锁 NetworkLock 超时防护| S[事务编排与校验 domain.rs / platform.rs]
  S -->|stdin JSON 无注入管道 + CREATE_NO_WINDOW| P[PowerShell 静默快照提取: IPv4 + IPv6 + DoH]
  S -->|可信 System32 路径 + CREATE_NO_WINDOW| N[netsh.exe 静默应用 IPv4/IPv6 地址与 DNS]
  S -->|可信 System32 路径 + CREATE_NO_WINDOW| D[PowerShell 静默配置 DoH 与 IPv6 绑定]
  D -->|读回深度比对校验| Check{校验是否完全生效}
  Check -- 校验失败或执行异常 --> R[自动安全回滚至修改前快照 (恢复 IPv4/IPv6/DNS/DoH)]
  Check -- 校验完全一致 --> V[确认成功并返回前端]
```

### 1. 技术选型
- **前端**：Vue 3 Composition API (`<script setup lang="ts">`) + TypeScript + Vite，提供现代响应式布局、Windows 11 Fluent 风格控件与无障碍表单体验。
- **后端/桌面容器**：Tauri 1 (Rust)，严格遵循系统安全与参数分离原则，打包为原生 Windows 轻量桌面应用。

### 2. 核心安全与可靠性设计
- **防代码注入 (A-01)**：所有外部进程均采用固定静态脚本，参数通过标准输入 (`stdin JSON`) 安全传输，不进行任何动态脚本文本拼接。
- **防止可执行文件劫持 (A-12)**：系统命令优先通过 `GetSystemDirectoryW` 定位 `SystemRoot\System32` 绝对路径 (`powershell.exe`、`netsh.exe`)，不依赖环境变量 `PATH`。
- **后台静默执行 (CREATE_NO_WINDOW)**：所有进程创建均附带 Windows 原生标志位 `0x08000000`，杜绝任何控制台弹窗与闪烁，保持内存占用在 35MB 左右。
- **作业对象孤儿清理 (Job Object)**：后端将外部子进程加入受管 Job Object，附带 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，主程序退出时立即强制回收子进程。
- **事务性变更与读回校验 (A-03)**：变更前先采集原配置全息快照（包含 IPv4、掩码、网关、常规 DNS、IPv6 启用状态、IPv6 地址/前缀/网关/主备 DNS 与 DoH 状态）；变更后主动读回系统配置确认生效；若步骤失败或读回不匹配，自动尝试还原现场，避免网络中断。
- **并发写锁与超时防死锁 (A-04)**：后端引入 `NetworkLock` 全局锁，写操作互斥串行化；耗时系统调用置于独立任务调度，并施加超时防挂起限制。
- **严格网络语义校验 (A-06)**：前后端对齐严谨的 IPv4 校验（拒绝前导零）、连续二进制掩码校验（拒绝 `255.0.255.0` 等非连续掩码）、网关同网段校验、IPv6 合法性格式校验以及 DoH HTTPS 协议安全校验。
- **网卡草稿独立绑定 (A-08)**：表单草稿与网卡身份绑定，切换网卡自动同步目标状态，杜绝跨网卡误配置。
- **历史记录安全隔离 (A-09, A-10)**：结构化校验版本 (`schemaVersion: 1`)，最多保留 10 条有效记录；本地存储异常仅提示警告，不误报网络配置失败。

---

## 三、项目结构说明

```
├── src/
│   ├── types/
│   │   └── network.ts            # 前端 DTO 接口定义 (含 DohConfig, AdapterSnapshot, Ipv4Config)
│   ├── utils/
│   │   └── validation.ts         # IPv4、连续掩码、网关子网及 IPv6 严谨校验
│   ├── services/
│   │   └── networkClient.ts      # 类型化 Tauri IPC 调用层
│   ├── composables/
│   │   ├── useNetworkConfig.ts   # 网卡选择、DoH / IPv6 状态草稿隔离与应用状态管理
│   │   └── useConfigHistory.ts   # 历史记录校验、持久化与异常隔离
│   ├── App.vue                   # 页面排版、Fluent 控件与交互组件
│   └── main.ts                   # 前端应用入口
├── src-tauri/
│   ├── src/
│   │   ├── domain.rs             # 领域模型、IPv4/IPv6 语义校验与快照比对算法
│   │   ├── platform.rs           # 可信路径、防注入管道、DoH/IPv6 管理与事务回滚
│   │   ├── lib.rs                # Tauri 命令注册与全局写锁
│   │   └── main.rs               # 后端主入口
│   └── tauri.conf.json           # Tauri 配置 (包含窗口尺寸、安全 CSP 及产物名称)
├── docs/                         # 审计与重构报告归档
├── releases/                     # 发行版输出目录 (包含便携版与 MSI)
├── package.json                  # 前端依赖与脚本
├── vite.config.ts                # Vite 配置
├── build-app.bat                 # 一键编译构建脚本
├── 打包说明.md                   # 便携版与安装包打包指南
└── README.md                     # 项目概览与使用文档
```

---

## 四、API 接口契约说明

### 1. 获取网络适配器列表
- **命令名**：`get_network_adapters`
- **入参**：无
- **出参**：`Vec<AdapterInfo>`
```typescript
interface AdapterInfo {
  name: string;             // 适配器名称 (如 "以太网")
  status: string;           // 格式化友好状态
  rawStatus?: string;       // 原始状态 (Up, Disconnected 等)
  displayName?: string;     // 设备描述
  interfaceIndex?: number;  // 接口索引
  interfaceGuid?: string;   // 接口唯一 GUID
  macAddress?: string;      // MAC 物理地址
}
```

### 2. 获取当前适配器完整快照
- **命令名**：`get_current_config`
- **入参**：`{ adapterName: string }`
- **出参**：`AdapterSnapshot`
```typescript
interface DohConfig {
  mode: 'off' | 'auto' | 'manual'; // DoH 模式
  template: string;                // HTTPS 模板 URL
  allowFallback: boolean;          // 是否允许降级为未加密 UDP 请求
}

interface AdapterSnapshot {
  adapterName: string;
  interfaceIndex: number;
  interfaceGuid: string;
  status: string;
  dhcpEnabled: boolean;
  dnsDhcpEnabled?: boolean;
  addresses: { ipAddress: string; prefixLength: number; mask: string }[];
  gateways: string[];
  dnsServers: string[];
  ip: string;
  mask: string;
  gateway: string;
  dns1: string;
  dns2: string;
  doh1?: DohConfig;
  doh2?: DohConfig;
  ipv6Enabled?: boolean;
  ipv6DhcpEnabled?: boolean;
  ipv6DnsDhcpEnabled?: boolean;
  ipv6Addresses?: { ipAddress: string; prefixLength: number }[];
  ipv6Gateways?: string[];
  ipv6DnsServers?: string[];
  ipv6Ip?: string;
  ipv6Prefix?: number;
  ipv6Gateway?: string;
  ipv6Dns1?: string;
  ipv6Dns2?: string;
}
```

### 3. 事务式应用网络配置
- **命令名**：`apply_adapter_ipv4_config`
- **入参**：`{ cfg: Ipv4Config }`
```typescript
interface Ipv4Config {
  adapter: string;
  ip: string;
  mask: string;
  gateway: string;
  dns1: string;
  dns2: string;
  doh1?: DohConfig;
  doh2?: DohConfig;
  ipv6Enabled?: boolean;
  ipMode?: 'dhcp' | 'static';
  dnsMode?: 'dhcp' | 'static';
  ipv6Mode?: 'dhcp' | 'static';
  ipv6Ip?: string;
  ipv6Prefix?: number;
  ipv6Gateway?: string;
  ipv6DnsMode?: 'dhcp' | 'static';
  ipv6Dns1?: string;
  ipv6Dns2?: string;
}
```
- **出参**：`OperationResult`
```typescript
interface OperationResult {
  success: boolean;            // 是否完全成功生效
  message: string;            // 详细结果说明
  rolledBack: boolean;        // 是否触发了安全回滚
  rollbackMessage?: string;   // 回滚状态描述
  snapshot?: AdapterSnapshot; // 读回校验后的最新状态快照
}
```

---

## 五、开发与构建

### 1. 安装依赖
```bash
npm install
```

### 2. 本地开发 (启动前端与桌面窗口)
```bash
npm run tauri dev
```

### 3. 前端类型检查与构建
```bash
npm run typecheck
npm run build
```

### 4. 后端编译与单元测试
```bash
cd src-tauri
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

### 5. 一键打包 Release 便携版
双击运行根目录下脚本：
```cmd
build-app.bat
```
编译产物位于：`src-tauri/target/release/Windows网络配置工具.exe`。
