# 🛠️ WindowsNetworkConfigTool_DevDoc_CN.md

# 📝 Windows网络配置工具开发文档（中文）

## 🏁 一、项目简介

本项目基于 [Tauri](https://tauri.app/) + [Vue3](https://vuejs.org/) 技术栈开发，旨在提供一个简单易用的 Windows 网络适配器 IPv4 配置工具。用户可通过图形界面查看、修改本机网络适配器的 IPv4 地址、子网掩码、网关及 DNS，并支持配置历史记录。

## 二、搭建与设计理念

### 1. 技术选型
- **前端**：Vue 3 + Vite，提供响应式、现代化的界面体验。
- **后端/桌面容器**：Tauri，利用 Rust 实现系统级操作，安全、高效，最终打包为原生 Windows 应用。

### 2. 设计理念
- **安全性**：Tauri 采用最小权限原则，tauri.conf.json 配置中仅允许必要的 API 权限。
- **易用性**：界面简洁直观，主要操作一目了然。
- **历史记录**：本地保存最近 10 条配置，便于快速切换。
- **跨平台**：虽然当前主要面向 Windows，但架构设计兼容其他平台。

## 三、主要功能实现

### 1. 网络适配器信息获取
- 通过 Tauri 后端（Rust）调用系统 API，获取所有网络适配器列表。
- 前端通过 `@tauri-apps/api/tauri` 的 `invoke` 方法调用 Rust 命令。

### 2. IPv4 配置修改
- 用户选择适配器，填写 IP、子网掩码、网关、DNS，点击“应用配置”按钮。
- 前端校验格式，调用 Tauri 后端命令，实际修改系统网络配置。

### 3. 历史配置管理
- 每次成功应用配置后，将该配置保存到 localStorage。
- 最多保留 10 条历史，支持一键应用历史配置。

### 4. 状态提示与异常处理
- 所有操作均有状态提示，失败时高亮显示错误信息。
- 异常静默处理，尽量不打断用户流程。

## 四、项目结构说明

```
├── src/              # 前端 Vue3 源码
│   ├── App.vue       # 主界面与核心逻辑
│   └── main.ts       # 入口文件
├── src-tauri/        # Tauri 后端（Rust）相关
│   └── tauri.conf.json # Tauri 配置
├── public/           # 静态资源
├── dist/             # 前端打包输出
├── package.json      # 前端依赖与脚本
├── vite.config.ts    # Vite 配置
├── README.md         # 简要说明
├── releases/         # 安装包
```

## 五、开发与运行

### 1. 安装依赖
```bash
yarn install
```

### 2. 本地开发
```bash
yarn tauri dev
```

### 3. 前端独立开发（不启用 Tauri，仅调试界面）
```bash
yarn dev
```

## 六、打包与发布

详细打包方法见 `打包说明.md`，核心流程如下：

### 1. 构建前端
```bash
yarn build
```

### 2. 构建/打包后端
```bash
cd src-tauri
cargo build --release
cd ..
# 或使用 Tauri CLI（生成便携版或MSI安装包）
yarn tauri build --target app
# 或
yarn tauri build --target msi
```

### 3. 其他说明
- 打包 32 位需先安装 32 位 Rust 工具链：`rustup target add i686-pc-windows-msvc`
- 打包 MSI 安装包需安装 [WiX Toolset](https://wixtoolset.org/releases/)
- 详细参数和注意事项见 `打包说明.md`

## 七、常见问题与建议
- **权限不足**：部分操作需管理员权限，建议以管理员身份运行。
- **依赖缺失**：确保已安装 Node.js、Yarn、Rust、Tauri CLI。
- **网络适配器列表为空**：请检查系统网络服务是否正常。

## 八、参考与致谢
- [Tauri 官方文档](https://tauri.app/zh-cn/docs/)
- [Vue 官方文档](https://cn.vuejs.org/)

---

## 九、API 交互与核心代码说明

### 1. 前端与后端通信（Tauri invoke）

```typescript
import { invoke } from '@tauri-apps/api/tauri';
const adapters = await invoke<AdapterInfo[]>("get_network_adapters");
const msg = await invoke<string>("apply_adapter_ipv4_config", { cfg: ipConfig });
const current = await invoke<IpConfig>("get_current_config", { adapter_name: selectedAdapter });
```

#### invoke 方法签名
```typescript
function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
```

### 2. 后端 Rust 命令接口说明

- `get_network_adapters`：返回所有网络适配器
- `get_current_config(adapter_name: String)`：返回 IPv4 配置
- `apply_adapter_ipv4_config(cfg: Ipv4Config)`：应用 IPv4 配置

#### 结构体示例
```rust
pub struct AdapterInfo {
    pub name: String,    // 适配器名称
    pub status: String,  // 显示名称（含状态）
}

pub struct Ipv4Config {
    pub adapter: String,
    pub ip: String,
    pub mask: String,
    pub gateway: String,
    pub dns1: String,
    pub dns2: String,
}
```

---

## 十、如何用 GitHub CLI 上传安装包到 GitHub Releases

1. **登录 GitHub CLI**
   ```bash
   gh auth login
   ```
   按提示选择 GitHub.com、HTTPS、浏览器授权。

2. **创建 Release 并上传安装包**
   ```bash
   gh release create v1.0.0 "releases/Network_adapter_IP4_information_modification (Run_with_administrator_privileges).exe" --title "v1.0.0" --notes "首个发布版本"
   ```
   - `v1.0.0` 是 tag 名，可自定义
   - `--title` 为 Release 标题
   - `--notes` 为 Release 描述

   或上传到已存在的 Release：
   ```bash
   gh release upload v1.0.0 "releases/Network_adapter_IP4_information_modification (Run_with_administrator_privileges).exe"
   ```

3. **检查结果**
   发布后访问你的仓库 Releases 页面即可下载。

> 注意：安装包名有空格时路径需用英文引号包裹。

---

如需进一步了解 Rust 后端实现，可参考 `src-tauri/src/lib.rs`，每个命令均有详细注释。
