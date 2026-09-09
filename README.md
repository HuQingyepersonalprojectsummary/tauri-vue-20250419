# 🚀 Windows 网络配置工具 (Windows Network Config Tool)

[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-blue.svg)](#)
[![Build With](https://img.shields.io/badge/build-tauri%20%2B%20vue3-brightgreen)](#)

> 💻 专业高效的 Windows 原生网络配置管理工具，支持 **IPv4、IPv6 与 DNS over HTTPS (DoH)**，具备秒级读回校验与自动回滚保护！

---

## 📚 目录
- 📝 [项目简介](#-项目简介)
- 🌟 [主要特性](#-主要特性)
- 📖 [架构与安全设计](#-架构与安全设计)
- ⚙️ [开发与打包](#️-开发与打包)
- 📑 [开发文档](#-开发文档)

---

## 📝 项目简介
本项目基于 **Tauri (Rust) + Vue 3 (TypeScript)** 技术栈构建，旨在为 Windows 用户提供轻量、安全、可靠且紧密贴合 Windows 11 原生规范的网络配置体验。工具通过结构化系统调用管理网卡，彻底消除了外部脚本注入隐患，并在变更失败或读回异常时自动回滚现场，最大程度保障网络可用性与配置一致性。

- 🖥️ **前端**：Vue 3 Composition API + TypeScript + Vite，现代化响应式界面，集成 Windows 11 Fluent 风格交互开关与公共 DNS 快捷预设。
- 🦀 **后端/容器**：Tauri 1 (Rust)，严格系统路径定位、安全管道、进程树超时管理、DoH 模板管理与跨进程互斥锁。

---

## 🌟 主要特性

- 🔒 **DNS over HTTPS (DoH) 加密解析**：
  - 首选与备用 DNS 独立支持 DoH 配置；
  - 提供 `关`、`开(自动)`、`开(手动模板)` 三种工作模式；
  - 支持自定义 HTTPS 加密模板 URL（如 DNSPod、阿里 DNS、Cloudflare 等），严格协议校验；
  - 具备“失败时使用未加密请求”的无缝回退（Fallback）控制。
- 🌐 **IPv6 协议组件安全绑定**：
  - 匹配 Windows 11 “编辑 IP 设置”规范，提供独立的 Fluent 风格拨动开关；
  - 一键启用或禁用目标网卡的 IPv6 (`ms_tcpip6`) 协议组件，且完整纳入快照保护。
- ⚡ **常用公共 DNS / DoH 快捷预设**：
  - 内置阿里公共 DNS、腾讯 DNSPod、Cloudflare、Google 等主流服务，支持一键填入地址与加密模板。
- 🛡️ **读回校验与自动事务回滚**：
  - 变更前采集完整快照（覆盖 IPv4、掩码、网关、DNS、IPv6 绑定与 DoH 配置）；
  - 应用后循环读回系统真实状态进行深度比对；
  - 一旦执行失败、超时或读回不符，立即自动回滚至修改前快照，杜绝断网风险。
- 💉 **零注入与可信执行路径**：
  - 外部进程参数均通过标准输入（stdin JSON）安全传递，不进行字符串动态拼接，免疫代码注入；
  - 系统命令定位于 `System32` 绝对路径，杜绝 PATH 劫持。
- 🎯 **多网卡独立草稿隔离**：
  - 切换网卡时自动持久化与恢复各网卡独立草稿，杜绝配置跨网卡串写。
- 🗂️ **安全历史预设**：
  - 带类型与版本校验的本地历史记录（上限 10 条），单项损坏自动隔离过滤，存储异常独立警告。

---

## 📖 架构与安全设计

- [项目审计文档 (2026-09-09)](./docs/项目审计文档-2026-09-09.md)：16 项审计发现及代码定位。
- [项目重构报告 (2026-09-09)](./docs/项目重构报告-2026-09-09.md)：渐进式重构路径、架构设计及验收标准。
- [开发文档 (中文)](./WindowsNetworkConfigTool_DevDoc_CN.md)：系统模块分层、DoH / IPv6 设计与 API 契约说明。
- [Developer Documentation (English)](./WindowsNetworkConfigTool_DevDoc_EN.md)：System architecture, API contracts, and safety guarantees.

---

## ⚙️ 开发与打包

### 🛠️ 1. 本地开发调试
```bash
# 1. 安装前端依赖
npm install

# 2. 启动桌面端调试
npm run tauri dev
```

### 🛠️ 2. 一键编译生成 64 位便携版 (绿色免安装)
在项目根目录下双击运行：
```cmd
build-app.bat
```
或通过命令行执行：
```bash
# 构建前端
npm run build

# 构建后端 Release
cd src-tauri
cargo build --release
cd ..
```
编译产物位于：
`src-tauri/target/release/tauri-vue-20250419.exe`

详细安装包 (MSI) 及 32 位架构打包指引请参阅：[打包说明.md](./打包说明.md)。
