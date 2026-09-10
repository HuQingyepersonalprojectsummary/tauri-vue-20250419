# Windows 网络配置工具

基于 Vue 3、TypeScript、Rust 和 Tauri 1 的 Windows 桌面应用，用于查看和修改网卡的 IPv4、IPv6、DNS 与系统 DoH 设置。当前版本为 **0.1.0**，本次构建目标为 **Windows x64**。

## 下载与运行

当前源码对应的程序保存在 [releases](./releases/)：

| 文件 | 用途 |
|---|---|
| [Windows网络配置工具.exe](./releases/Windows网络配置工具.exe) | 便携程序 |
| [Windows_Network_Config_Tool_v0.1.0.exe](./releases/Windows_Network_Config_Tool_v0.1.0.exe) | 同一便携程序的英文文件名副本 |
| [Windows网络配置工具_0.1.0_x64-setup.exe](./releases/Windows网络配置工具_0.1.0_x64-setup.exe) | NSIS 安装包 |
| [Windows网络配置工具_0.1.0_x64_zh-CN.msi](./releases/Windows网络配置工具_0.1.0_x64_zh-CN.msi) | 中文 MSI 安装包 |

下载后可用 [SHA256SUMS.txt](./releases/SHA256SUMS.txt) 校验文件；[release-manifest.json](./releases/release-manifest.json) 记录产物及源码指纹。发布文件未配置代码签名。本次沿用 0.1.0，以 SHA-256 区分旧构建。

运行需要 Microsoft Edge WebView2 Runtime。安装包配置为按需下载 WebView2 引导程序，缺少运行时时需要联网；便携版也依赖已安装的 WebView2。修改系统网络需要管理员权限，请右键程序选择“以管理员身份运行”。安装器提权不代表应用启动会自动提权。

## 功能与使用

1. 选择目标网卡，读取当前状态，确认网卡名称。对于未绑定或显式禁用 IPv6 的网卡（如虚拟网卡或专用通道），程序自动识别并优雅回退，不阻断常规 IPv4 与 DNS 读取。
2. 分别选择 IPv4 地址、IPv4 DNS、IPv6 地址和 IPv6 DNS 的“保持现状”“自动”或“手动”模式。需要修改的部分才选择主动配置。
3. 手动 IPv4 要求合法地址及连续子网掩码，填写网关时要求与地址处于同一子网。IPv6 前缀支持 1–128，手动 IPv6 DNS 的首选服务器必填、备用可选。
4. 可选择 DNS 预设，或设置 IPv4 DNS 对应的 DoH 自动/手动模板及明文回退选项。系统必须支持相关 DoH 命令；DoH 服务器条目是系统级设置，可能影响其他网卡。
5. 应用后查看操作结果和读回快照。应用失败时程序尝试补偿并核验恢复状态；如果显示恢复失败或现场未知，应核对 Windows 网络设置。

配置历史保存在当前应用 WebView 的 localStorage 中，最多 10 条；网卡切换草稿只保存在本次运行的内存中。旧历史未指定的 IPv6 模式继续保持现状。历史保存失败会单独提示，不改变网络操作的成功结果。

回滚是逐条命令补偿，无法保证所有故障下完整恢复或网络不中断。当前验证范围及未完成工作见 [验证记录](./docs/verification.md) 和 [已知限制](./docs/known-limitations.md)。

## 开发

本次使用 Node.js 24.16.0、Rust/Cargo 1.91.1、PowerShell 7 和 Windows MSVC 构建环境。前端以 `yarn.lock`、Rust 以 `src-tauri/Cargo.lock` 为依赖依据。

```powershell
# 使用项目约定的 Yarn Classic，避免额外生成 package-lock.json
npx --yes yarn@1.22.22 install --frozen-lockfile
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
npm run tauri -- dev
```

`npm run dev` 仅启动浏览器界面，不能调用原生 IPC。桌面调试使用 `npm run tauri -- dev`；开发服务器固定端口为 3000。

```powershell
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
npm run test:regression
npm run release
```

`npm run release` 或 `build-app.bat` 构建前端、原生程序、MSI 和 NSIS，随后复制当前产物并生成校验文件。首次打包可能需要下载 WiX/NSIS 工具。具体环境准备及故障处理见 [打包说明](./打包说明.md)。

## 文档

- [中文开发文档](./WindowsNetworkConfigTool_DevDoc_CN.md)：模块、IPC、数据契约、事务和维护流程。
- [English developer guide](./WindowsNetworkConfigTool_DevDoc_EN.md)：英文开发参考。
- [打包说明](./打包说明.md)：构建、产物核验和安装验收。
- [文档索引](./docs/README.md)：验证结论、已知限制和后续工作。
- [回归测试说明](./tests/regression/ipv6/README.md)：隔离探针及覆盖范围。
