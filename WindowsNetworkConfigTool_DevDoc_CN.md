# 🛠️ WindowsNetworkConfigTool_DevDoc_CN.md

# 📝 Windows网络配置工具开发文档（中文）

## 🏁 一、项目简介

本项目基于 [Tauri](https://tauri.app/) + [Vue 3](https://vuejs.org/) 技术栈开发，为 Windows 用户提供安全、直观、可靠的网络适配器配置工具。用户可通过图形界面查看、修改本机网络适配器的：
- **IPv4 地址与掩码**（支持严格连续二进制掩码校验与零前导校验）
- **默认网关**（支持同网段可达性校验）
- **常规 DNS 服务器**（首选与备用 DNS 依赖关系校验）
- **DNS over HTTPS (DoH) 加密解析**（开/关、开(自动)、开(手动模板) 及未加密请求回退）
- **IPv6 协议组件绑定状态**（安全启用或禁用适配器 `ms_tcpip6` 协议）
- **常用公共 DNS / DoH 一键预设**（阿里 DNS、腾讯 DNSPod、Cloudflare、Google）
- **带版本校验与安全持久化的配置历史记录**（上限 10 条）

---

## 二、架构设计与安全理念

```mermaid
flowchart TD
  UI[Vue 3 前端界面] -->|类型化 IPC| IPC[Tauri Commands]
  IPC -->|串行写锁 NetworkLock| S[事务编排与校验 domain.rs]
  S -->|stdin JSON 无注入管道| P[PowerShell 快照提取: IPv4 + IPv6 + DoH]
  S -->|可信 System32 路径| N[netsh.exe 应用 IPv4 地址与网关]
  S -->|可信 System32 路径| D[PowerShell 配置 DoH 与 IPv6 绑定]
  D -->|读回校验或步骤失败| R[自动安全回滚至修改前快照 (含 IPv4/IPv6/DoH)]
  D -->|读回校验完全一致| V[确认成功并返回前端]
```

### 1. 技术选型
- **前端**：Vue 3 Composition API (`<script setup lang="ts">`) + TypeScript + Vite，提供现代响应式布局、Windows 11 Fluent 风格控件与无障碍表单体验。
- **后端/桌面容器**：Tauri 1 (Rust)，严格遵循系统安全与参数分离原则，打包为原生 Windows 轻量桌面应用。

### 2. 核心安全与可靠性设计
- **防代码注入 (A-01)**：所有外部进程均采用固定静态脚本，参数通过标准输入 (stdin JSON) 安全传输，不进行任何动态脚本文本拼接。
- **防止可执行文件劫持 (A-12)**：系统命令优先定位 `SystemRoot\System32` 绝对路径 (`powershell.exe`、`netsh.exe`)，不依赖环境变量 `PATH`。
- **事务性变更与读回校验 (A-03)**：变更前先采集原配置完整快照（包含 IPv4、网关、DNS、IPv6 绑定与 DoH 状态）；变更后主动读回系统配置确认生效；若步骤失败或读回不匹配，自动尝试还原现场，避免网络中断。
- **并发写锁与异步防阻塞 (A-04)**：后端引入 `NetworkLock` 全局锁，写操作互斥串行化；耗时系统调用置于独立任务调度，并施加超时回收限制。
- **严格网络语义校验 (A-06)**：前后端对齐严谨的 IPv4 校验（拒绝前导零）、连续二进制掩码校验（拒绝 `255.0.255.0` 等无效掩码）、网关同网段校验以及 DoH HTTPS 协议安全校验。
- **网卡草稿独立绑定 (A-08)**：表单草稿与网卡身份绑定，切换网卡自动同步目标状态，杜绝跨网卡误配置。
- **历史记录安全隔离 (A-09, A-10)**：结构化校验版本 (`schemaVersion: 1`)，最多保留 10 条有效记录；本地存储异常仅提示警告，不误报网络配置失败。

---

## 三、项目结构说明

```
├── src/
│   ├── types/
│   │   └── network.ts            # 前端 DTO 接口定义 (含 DohConfig, AdapterSnapshot)
│   ├── utils/
│   │   └── validation.ts         # IPv4、连续掩码及网关子网校验
│   ├── services/
│   │   └── networkClient.ts      # 类型化 Tauri IPC 调用层
│   ├── composables/
│   │   ├── useNetworkConfig.ts   # 网卡选择、DoH / IPv6 状态草稿隔离与应用状态管理
│   │   └── useConfigHistory.ts   # 历史记录校验、持久化与异常隔离
│   ├── App.vue                   # 页面排版、Fluent 控件与交互组件
│   └── main.ts                   # 前端应用入口
├── src-tauri/
│   ├── src/
│   │   ├── domain.rs             # 领域模型、DoH 与 IPv4 算法与单元测试
│   │   ├── platform.rs           # 可信路径、防注入脚本、DoH/IPv6 管理与事务回滚
│   │   ├── lib.rs                # Tauri 命令注册与全局写锁
│   │   └── main.rs               # 后端主入口
│   └── tauri.conf.json           # Tauri 配置 (含安全 CSP)
├── docs/                         # 审计与重构报告归档
├── releases/                     # 便携版可执行程序输出目录
├── package.json                  # 前端依赖与脚本
├── vite.config.ts                # Vite 配置
├── build-app.bat                 # 一键编译构建脚本
├── 打包说明.md                   # 便携版与安装包打包指南
└── README.md                     # 项目概览
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
  addresses: { ipAddress: string; prefixLength: number; mask: string }[];
  gateways: string[];
  dnsServers: string[];
  ip: string;
  mask: string;
  gateway: string;
  dns1: string;
  dns2: string;
  doh1?: DohConfig;       // 首选 DNS 的 DoH 状态
  doh2?: DohConfig;       // 备用 DNS 的 DoH 状态
  ipv6Enabled?: boolean;  // IPv6 组件 (ms_tcpip6) 是否启用
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
  gateway: string;        // 可选，留空时不配置默认网关
  dns1: string;           // 可选，留空时不修改 DNS
  dns2: string;           // 可选
  doh1?: DohConfig;       // 首选 DoH 配置
  doh2?: DohConfig;       // 备用 DoH 配置
  ipv6Enabled?: boolean;  // IPv6 绑定开关
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
# 或使用 yarn:
# yarn install
```

### 2. 本地开发 (启动前端与桌面窗口)
```bash
npm run tauri dev
# 或:
# yarn tauri dev
```

### 3. 前端独立开发调试
```bash
npm run dev
```

### 4. 前端类型检查与构建
```bash
npm run typecheck
npm run build
```

### 5. 后端编译与单元测试
```bash
cd src-tauri
cargo test
cargo check
```

### 6. 一键打包 Release 便携版
运行根目录下脚本：
```cmd
build-app.bat
```
或执行：
```bash
npm run build
cd src-tauri && cargo build --release
```
编译产物位于 `src-tauri/target/release/tauri-vue-20250419.exe`。
