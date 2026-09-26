# Themida 商业级严苛加壳与发布指南

本文档详细说明如何使用位于 `C:\BLWJ\Themida_3.2.6.0_nCh2CZZRNLW` 的 Themida 3.2.6.0 (x64) 为本项目的 **Windows 网络配置工具** 打包构建**最高安全标准、最严苛防护的商业化发布包**。

---

## 1. 商业发布交付包规格 (Commercial Delivery Spec)

运行商业发布打包流水线后，产物集中输出至 `releases/commercial/` 目录：

| 文件名称 | 格式与架构 | 说明与防护标准 |
|---|---|---|
| `Windows网络配置工具_v0.1.0_x64_Portable_Commercial.exe` | x64 便携单文件 | **Themida 最严苛商业加壳**：多虚拟机代码虚拟化、反调试、防内存转储、IAT混淆、UAC自动提权 |
| `Windows网络配置工具_0.1.0_x64-setup.exe` | NSIS 安装程序 | 商业安装包（包含桌面快捷方式、开始菜单图标、完整卸载程序及管理员权限配置） |
| `Windows网络配置工具_0.1.0_x64_zh-CN.msi` | MSI 企业级安装包 | 专为企业 GPO / 域控环境静默批量分发设计 |
| `SHA256SUMS.txt` | 纯文本 | 所有商业交付产物的 SHA-256 哈希校验清单 |
| `commercial-manifest.json` | JSON 结构化清单 | 包含构建时间戳、文件大小、哈希、源码指纹及加壳安全特性声明 |

---

## 2. 最严苛商业化加壳策略配置清单 (Themida Strict Profile)

在 Themida 图形界面或工程文件（`scripts/themida.tmd`）中，请按照以下指标进行**最严格商业化防护配置**：

### 2.1 [Application Information]（基础配置）
- **Input Filename**：`releases\Windows网络配置工具.exe`
- **Output Filename**：`releases\commercial\Windows网络配置工具_v0.1.0_x64_Portable_Commercial.exe`

### 2.2 [Protection Options]（核心反逆向与防破解）
- ✅ **Anti-Debugger Protection（多维反调试）**：
  - 勾选 **Detect Debuggers**（实时探测 Ring3/Ring0 调试器，如 x64dbg、IDA Pro、Cheat Engine、Process Hacker）。
  - 勾选 **Hide from Debuggers**（向 Windows 内核隐藏主线程 `ThreadHideFromDebugger`，阻止调试器附加）。
  - 勾选 **Anti-Attach**（禁止任意外部进程注入与挂钩）。
- ✅ **Anti-Dump Protection（防内存转储与脱壳）**：
  - 勾选 **Erase PE Header from Memory after unpack**（解密执行后立即在内存中擦除 PE 头，使 Scylla / MegaDumper 彻底无法还原 PE 结构与节表）。
- ✅ **API Protection & Import Table Obfuscation（导入表隐藏）**：
  - 勾选 **Advanced API-Wrapping**（系统 API 调用定向至保护壳内部多态桩代码，消除明文 IAT，隐藏底层 Win32 / netsh 机制）。
- ✅ **Anti-Patching & Code Integrity（防代码补丁与完整性验证）**：
  - 勾选 **Memory Patch Detection**（周期性扫描内存代码区，拦截软硬件断点及内存篡改）。
  - 勾选 **File Integrity Check**（对可执行文件进行全盘哈希校验，防止二进制修改）。
- ✅ **Anti-Monitor（防系统监控）**：
  - 勾选 **Anti-API Monitor / Anti-Procmon**（防御 API Monitor 与行为分析沙箱）。
- ⚠️ **Resource Protection（资源保留）**：
  - 保持 **不深度压缩关键资源（Keep Icon/Version Info）**，确保 Windows 资源管理器与任务栏能正常渲染高清产品图标。

### 2.3 [Virtual Machine]（多态代码虚拟化）
- ✅ 勾选 **Entry Point Virtualization**（入口点直接在虚拟机内执行）。
- 虚拟机引擎推荐选择 **Fish (Extreme)** 或 **Tiger (Extreme)** 多态虚拟机，将 x64 机器码彻底转为私有字节码解释执行，反编译器（IDA Pro / Ghidra）无法生成反编译代码。

### 2.4 [Extra Options / UAC]（商业级用户体验）
- ✅ **Manifest 权限注入**：设置/添加 Manifest 为 **`requireAdministrator`**。
  - **重要原因**：网络适配器（IP、掩码、网关、DNS、DoH）的修改必须依赖 Windows 管理员特权。注入该清单后，用户双击直接触发 UAC 弹窗提权，不再需要繁琐的“右键以管理员身份运行”。

### 2.5 架构级安全放行
- **Edge WebView2 兼容**：Tauri 需要唤起 `msedgewebview2.exe` 子进程，切勿勾选“终止未知子进程”策略，确保用户界面毫秒级无障碍呈现。
- **SEH 结构化异常**：保持 SEH 开启，保障 Rust panic unwind 与 Windows 异常链稳定。

---

## 3. 商业打包一键执行命令

### 一键执行全套商业构建流水线
在仓库根目录直接运行批处理或 npm 脚本：

```cmd
# 方式 A：双击或在终端执行批处理
build-commercial.bat

# 方式 B：通过 npm 运行
npm run build:commercial
```

### 首次配置工程文件：
如果尚未保存 `scripts/themida.tmd`，运行脚本后会自动为您打开 Themida 图形界面，您只需在界面上按照上述第 2 节的清单配置并点击 **Save Project** 保存到 `scripts/themida.tmd`，然后点击 **Protect** 即可。

后续所有打包将**全自动通过命令行静默加壳**，直接输出到 `releases/commercial` 交付目录！
