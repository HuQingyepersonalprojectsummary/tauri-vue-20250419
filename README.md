# Windows 网络配置工具 (Windows Network Config Tool)

<div align="center">

![Tauri](https://img.shields.io/badge/Tauri-1.5-24C8D8?style=flat-square&logo=tauri&logoColor=white)
![Vue 3](https://img.shields.io/badge/Vue-3.5-4FC08D?style=flat-square&logo=vuedotjs&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-5.8-3178C6?style=flat-square&logo=typescript&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-1.91-black?style=flat-square&logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/Platform-Windows%20x64-0078D4?style=flat-square&logo=windows&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-brightgreen?style=flat-square)

<p>基于 Vue 3、TypeScript、Rust 与 Tauri 1 构建的现代化 Windows 网络适配器管理工具。<br/>提供高可靠的 IPv4、IPv6、DNS、DoH 配置读写、特权前置校验与自动化逆向事务补偿。</p>

<img src="./assets/PixPin_2025-04-22_20-35-34.png" alt="Windows 网络配置工具界面预览" width="460" style="border-radius: 8px; box-shadow: 0 4px 16px rgba(0,0,0,0.15);" />

</div>

---

## 🌟 核心特性

- 🌐 **IPv4 / IPv6 双栈独立控制**：支持分别针对 IPv4 地址、IPv4 DNS、IPv6 地址及 IPv6 DNS 进行“保持现状”、“自动 (DHCP/SLAAC)”或“手动静态”配置，仅对明确选中的项进行修改。
- 🛡️ **逆向补偿事务与精确读回核验**：执行任何修改前自动留存网卡物理状态快照。配置失败或超时即刻启动逆向补偿序列，并在补偿后强制读回系统物理现场；绝不将未完全恢复伪报为成功。
- 🔒 **DNS over HTTPS (DoH) 加密解析**：内置常用公共 DNS 预设，支持一键配置系统级 DoH 自动升级模式、自定义 HTTPS 模板及明文降级回退选项。
- ⚡ **无黑框静默执行与进程治理**：底层原生调用 PowerShell 与 netsh 均配置 `CREATE_NO_WINDOW` 抑制控制台弹窗，并通过 Windows Job Object（作业对象）杜绝超时或崩溃引发的孤儿子进程。
- 🔐 **跨进程并发锁与特权前置防护**：采用 Windows 全局命名互斥体（Global Named Mutex）避免多实例竞态修改；写操作前主动检测管理员权限凭据，普通权限优雅阻断并提示提权。
- 🔄 **禁用 IPv6 网卡平滑兼容**：针对虚拟网卡或显式禁用 IPv6 协议栈的适配器，底层 CIM 查询自动容错回退，保障常规 IPv4 与 DNS 读写不受阻断。
- 📝 **配置历史与多网卡草稿**：本地保存最近 10 条有效历史配置；界面切换网卡时自动暂存当前编辑草稿，避免重复输入。

---

## 📦 下载与运行

当前版本为 **v0.1.0**，构建目标为 **Windows x64**。预编译发布文件存放在 [releases/](./releases/)：

| 文件名称 | 格式类型 | 说明 |
|---|---|---|
| [Windows网络配置工具.exe](./releases/Windows网络配置工具.exe) | 便携免安装版 | 绿色单文件，解压即用 |
| [Windows_Network_Config_Tool_v0.1.0.exe](./releases/Windows_Network_Config_Tool_v0.1.0.exe) | 便携免安装版 (英文名) | 与上述便携版内容一致，供脚本或英文环境引用 |
| [Windows网络配置工具_0.1.0_x64-setup.exe](./releases/Windows网络配置工具_0.1.0_x64-setup.exe) | NSIS 安装包 | 标准 Windows 桌面安装向导 |
| [Windows网络配置工具_0.1.0_x64_zh-CN.msi](./releases/Windows网络配置工具_0.1.0_x64_zh-CN.msi) | MSI 安装包 | 企业部署及 Windows Installer 规范安装包 |

### 校验和与完整性
- 完整性校验哈希表见 [releases/SHA256SUMS.txt](./releases/SHA256SUMS.txt)。
- 构建输入源指纹与构建元数据记录于 [releases/release-manifest.json](./releases/release-manifest.json)。

> [!IMPORTANT]
> - **运行依赖**：需要系统中已安装 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（Windows 10/11 系统通常已内置）。
> - **管理员权限**：Windows 操作系统限制网络适配器与 DNS 修改必须具备管理员特权。请右键程序选择**“以管理员身份运行”**。

---

## 🛠️ 本地开发与构建

### 1. 环境准备
- 操作系统：Windows x64
- Shell：PowerShell 7 (`pwsh`)
- 运行时与编译链：Node.js >= 20、Rust/Cargo >= 1.75、Visual Studio C++ Build Tools 与 Windows SDK

```powershell
# 使用项目约定的 Yarn Classic 安装前端依赖
npx --yes yarn@1.22.22 install --frozen-lockfile

# 预取并锁定 Rust 原生依赖
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
```

### 2. 启动桌面端调试
```powershell
# 启动 Tauri 桌面端热重载开发环境（Vite 固定监听 3000 端口）
npm run tauri -- dev
```
> `npm run dev` 仅启动网页端，无法调用 Tauri 底层原生 IPC。

### 3. 代码规范、单元测试与回归测试
```powershell
# 前端 TypeScript 严格类型检查
npm run typecheck

# Rust 代码格式检查
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check

# Rust 零警告静态检查
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings

# Rust 单元测试（包含网络验证、锁机制、netsh 参数及提权检测等 17 项用例）
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline

# 有状态事务逆向补偿与故障注入回归测试（12 项场景）
npm run test:regression
```

### 4. 一键打包发布
```powershell
# 一键生成前端、便携 EXE、MSI 和 NSIS，并导出至 releases 目录
npm run release
```
也可双击运行仓库根目录下的 [build-app.bat](./build-app.bat)。详细打包指引与故障排除见 [打包说明](./打包说明.md)。

---

## 📚 详细文档导航

| 文档名称 | 内容描述 |
|---|---|
| 📖 [文档导航中心 (docs/README.md)](./docs/README.md) | 全套文档索引与常用命令速查。 |
| 🛠️ [中文开发文档 (DevDoc_CN.md)](./WindowsNetworkConfigTool_DevDoc_CN.md) | 深度讲解系统分层架构、IPC 数据契约、事务补偿与状态机设计。 |
| 🌐 [English Developer Guide (DevDoc_EN.md)](./WindowsNetworkConfigTool_DevDoc_EN.md) | 英文版系统设计架构、IPC 接口规范与开发维护指南。 |
| 📦 [打包发布说明 (打包说明.md)](./打包说明.md) | 环境配置、安装包构建机制、哈希指纹校验与验收测试规范。 |
| 🛡️ [已知限制与技术边界 (docs/known-limitations.md)](./docs/known-limitations.md) | 说明 UAC 提权机制、非原子事务恢复边界、DoH 系统级影响与系统约束。 |
| 🧪 [验证记录与测试报告 (docs/verification.md)](./docs/verification.md) | 涵盖 17 项单元测试、12 项有状态回归测试、审计整改闭环与推荐验收清单。 |
| 🔬 [回归测试说明 (tests/regression/ipv6/README.md)](./tests/regression/ipv6/README.md) | 隔离探针设计、受控 IO 故障模拟与回归测试执行机制。 |
| 📄 [开源许可证 (LICENSE)](./LICENSE) | MIT 开源许可证文本。 |

---

## 📄 开源许可证

本项目基于 [MIT License](./LICENSE) 协议开源。
