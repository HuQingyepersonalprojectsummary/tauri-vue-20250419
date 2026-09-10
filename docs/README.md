# Windows 网络配置工具文档中心

欢迎查阅 **Windows 网络配置工具**（Windows Network Config Tool）技术文档中心。本工具基于 Vue 3、TypeScript、Rust 和 Tauri 1 构建，专为 Windows x64 系统提供高性能、可靠的网络适配器配置管理。

---

## 📚 文档导航

| 文档 | 描述 |
|---|---|
| 📖 [项目主页 (README.md)](../README.md) | 项目简介、快速上手、发布物下载与主要特性速览。 |
| 🛠️ [中文开发文档](../WindowsNetworkConfigTool_DevDoc_CN.md) | 深入讲解系统架构、模块职责、IPC 契约、事务补偿与状态机设计。 |
| 🌐 [English Developer Guide](../WindowsNetworkConfigTool_DevDoc_EN.md) | 英文版系统架构设计与开发者开发指南。 |
| 📦 [打包与发布说明](../打包说明.md) | 详细的环境搭建、便携包/MSI/NSIS 打包、校验流程与故障排除。 |
| 🛡️ [已知限制与技术边界](./known-limitations.md) | 明确说明 UAC 提权机制、事务回滚边界、DoH 系统级影响及底层系统约束。 |
| 🧪 [验证记录与测试报告](./verification.md) | 记录自动化测试、有状态故障注入测试、审计整改闭环与实机验收清单。 |
| 🔬 [回归测试套件说明](../tests/regression/ipv6/README.md) | 详细介绍隔离测试夹具、故障注入探针设计与回归用例执行方式。 |
| 📄 [开源许可证 (LICENSE)](../LICENSE) | MIT 开源协议文本。 |

---

## 🚀 常用开发命令速查

在仓库根目录下，使用推荐的统一工具链执行日常命令：

```powershell
# 1. 安装前端锁定依赖 (Yarn Classic 1.22.22)
npx --yes yarn@1.22.22 install --frozen-lockfile

# 2. 预先拉取并锁定 Rust 依赖
cargo fetch --manifest-path src-tauri/Cargo.toml --locked

# 3. 启动桌面端调试 (Tauri + Vue 3 实时热重载)
npm run tauri -- dev

# 4. 执行静态检查与代码质量把控
npm run typecheck                                                                     # 前端 TypeScript 严格检查
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check                            # Rust 格式检查
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings # Rust Clippy 零警告检查

# 5. 执行单元测试与有状态回归测试
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline                   # Rust 领域逻辑与安全锁测试
npm run test:regression                                                              # 有状态事务与故障注入回归测试
npm run test:functional                                                              # 功能逻辑与意图保持回归测试 (18 项)

# 6. 一键构建并导出全部发布文件 (便携 EXE、MSI、NSIS、校验和及指纹)
npm run release
```

---

## 🏛️ 核心架构与设计原则

1. **领域与系统隔离**：
   - `domain.rs` 包含纯 Rust 领域实体、DTO、合法性校验与快照比对算法，不包含任何外部系统 IO，具备高可测试性；
   - `platform.rs` 封装全部底层 Windows PowerShell、netsh、Job Object 及命名互斥体操作。
2. **防竞态与静默执行**：
   - 跨进程使用命名互斥体（Named Mutex），进程内使用异步排队锁；
   - 系统子进程统一配置 `CREATE_NO_WINDOW` 抑制控制台黑框，并通过 Job Object 避免孤儿进程残留。
3. **逆向事务补偿（Compensating Rollback）**：
   - 任何配置写入前采集完整网卡快照；
   - 写入失败或超时即刻启动逆向补偿；
   - 补偿后再次读回核验物理状态，若未能彻底恢复则向用户呈现完整错误诊断，严禁伪报恢复成功。
