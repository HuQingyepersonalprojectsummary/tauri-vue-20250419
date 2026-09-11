# Windows 网络配置工具 (Windows Network Config Tool)

![Tauri](https://img.shields.io/badge/Tauri-1.5-24C8D8?style=flat-square\&logo=tauri\&logoColor=white)
![Vue 3](https://img.shields.io/badge/Vue-3.4-4FC08D?style=flat-square\&logo=vuedotjs\&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-5.8-3178C6?style=flat-square\&logo=typescript\&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-1.91-black?style=flat-square\&logo=rust\&logoColor=white)
![Platform](https://img.shields.io/badge/Platform-Windows%20x64-0078D4?style=flat-square\&logo=windows\&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-brightgreen?style=flat-square)

一款基于 **Vue 3 + TypeScript + Rust + Tauri** 开发的 Windows 网络配置工具。

用于管理 Windows 网络适配器的 **IPv4、IPv6、DNS 与 DNS over HTTPS（DoH）** 配置，并提供配置校验、失败恢复、历史记录等功能。

---

## ✨ 功能介绍

### 🌐 IPv4 / IPv6 配置

支持分别管理网络适配器的 IPv4 和 IPv6 配置。

针对不同配置项，可以选择：

* **保持现状**
* **自动获取**
* **手动配置**

IPv4 地址、IPv4 DNS、IPv6 地址和 IPv6 DNS 可以分别设置。

程序只修改用户明确选择的配置项，未选择修改的项目将尽量保持原有配置。

### 🔒 DNS over HTTPS（DoH）

支持 Windows 系统级 DNS over HTTPS 配置。

主要功能包括：

* 常用公共 DNS 服务器预设
* DoH 自动升级模式
* 自定义 DoH HTTPS 模板
* 明文 DNS 回退配置

### 🛡️ 配置失败恢复

在修改网络配置前，程序会保存当前网络适配器的相关状态。

如果配置过程中发生失败或超时，程序会尝试按照修改前保存的状态恢复相关配置，并重新读取 Windows 中的实际网络状态进行核验。

> [!NOTE]
> 网络配置涉及 Windows 网络协议栈、网络适配器驱动以及多个系统组件，因此恢复过程仍可能受到操作系统状态、驱动程序或第三方网络软件影响。

### ⚡ 静默执行系统命令

底层调用 PowerShell、`netsh` 等 Windows 系统工具时采用无控制台窗口方式运行，避免执行网络配置过程中频繁弹出命令行窗口。

同时使用 Windows Job Object 管理相关子进程，降低命令超时或程序异常退出后遗留后台进程的可能性。

### 🔐 管理员权限检查

修改 Windows 网络适配器、IP 地址和 DNS 配置通常需要管理员权限。

程序会在执行写操作前检查当前权限。

如果权限不足，将阻止相关操作并提示用户使用管理员权限运行程序。

### 🔄 多种网络适配器兼容

程序对部分特殊网络环境进行了兼容处理，包括：

* IPv6 被禁用的网络适配器
* 虚拟网络适配器
* IPv4 / IPv6 状态不完整的网络环境

当 IPv6 查询不可用时，程序会进行相应的容错处理，尽量避免影响 IPv4 和 DNS 配置功能。

### 📝 配置历史与编辑草稿

支持保存最近 **10 条有效网络配置记录**。

切换不同网络适配器时，当前正在编辑的配置内容会暂时保存，减少重复输入。

---

## 📸 界面预览
