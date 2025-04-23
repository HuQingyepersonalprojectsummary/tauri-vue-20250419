# 🛠️ WindowsNetworkConfigTool_DevDoc_EN.md

# 📝 Windows Network Configuration Tool - Developer Documentation (English)

## 🏁 1. Project Overview

This project is built with [Tauri](https://tauri.app/) + [Vue3](https://vuejs.org/), aiming to provide a simple and user-friendly Windows network adapter IPv4 configuration tool. Users can view and modify IPv4 addresses, subnet masks, gateways, and DNS for local network adapters via a GUI, with history support.

## 2. Setup & Design Philosophy

### 2.1 Technology Stack
- **Frontend**: Vue 3 + Vite for a modern, reactive UI.
- **Backend/Desktop**: Tauri (Rust) for secure, efficient system-level operations, packaged as a native Windows app.

### 2.2 Design Principles
- **Security**: Minimal permission principle in Tauri (`tauri.conf.json` only allows necessary APIs).
- **Usability**: Clean and intuitive UI.
- **History**: Stores up to 10 recent configurations locally for quick switching.
- **Cross-platform**: Designed for Windows but extensible to other platforms.

## 3. Main Features

### 3.1 Adapter Info Retrieval
- Backend (Rust) fetches all network adapters via system APIs.
- Frontend calls backend using `@tauri-apps/api/tauri`'s `invoke` method.

### 3.2 IPv4 Configuration
- User selects adapter, fills IP/mask/gateway/DNS, clicks "Apply".
- Frontend validates, backend applies config via system commands.

### 3.3 History Management
- Each successful config is saved to localStorage (max 10 entries).
- One-click apply from history.

### 3.4 Status & Error Handling
- All actions have status feedback, errors highlighted.
- Errors are handled gracefully.

## 4. Project Structure

```
├── src/              # Frontend Vue3 source
│   ├── App.vue       # Main UI & logic
│   └── main.ts       # Entry
├── src-tauri/        # Tauri backend (Rust)
│   └── tauri.conf.json # Tauri config
├── public/           # Static assets
├── dist/             # Frontend build output
├── package.json      # Frontend deps/scripts
├── vite.config.ts    # Vite config
├── README.md         # Brief intro
├── releases/         # Installers
```

## 5. Development & Run

### 5.1 Install dependencies
```bash
yarn install
```

### 5.2 Local development
```bash
yarn tauri dev
```

### 5.3 Frontend only (UI debug)
```bash
yarn dev
```

## 6. Build & Release

See detailed steps in `打包说明.md` (Packaging Guide, Chinese). Core steps:

### 6.1 Build frontend
```bash
yarn build
```

### 6.2 Build backend
```bash
cd src-tauri
cargo build --release
cd ..
# Or use Tauri CLI for portable/MSI:
yarn tauri build --target app
# or
yarn tauri build --target msi
```

### 6.3 Notes
- 32-bit: `rustup target add i686-pc-windows-msvc`
- MSI: Install [WiX Toolset](https://wixtoolset.org/releases/)
- See `打包说明.md` for more

## 7. FAQ & Suggestions
- **Permission denied**: Run as administrator.
- **Missing dependencies**: Ensure Node.js, Yarn, Rust, Tauri CLI are installed.
- **No adapters listed**: Check Windows network services.

## 8. References
- [Tauri Docs](https://tauri.app/zh-cn/docs/)
- [Vue Docs](https://cn.vuejs.org/)

---

## 9. API Interaction & Core Code

### 9.1 Frontend-Backend Communication (Tauri invoke)

```typescript
import { invoke } from '@tauri-apps/api/tauri';
const adapters = await invoke<AdapterInfo[]>("get_network_adapters");
const msg = await invoke<string>("apply_adapter_ipv4_config", { cfg: ipConfig });
const current = await invoke<IpConfig>("get_current_config", { adapter_name: selectedAdapter });
```

#### invoke signature
```typescript
function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
```

### 9.2 Rust Backend Commands

- `get_network_adapters`: Returns all network adapters
- `get_current_config(adapter_name: String)`: Returns IPv4 config
- `apply_adapter_ipv4_config(cfg: Ipv4Config)`: Applies IPv4 config

#### Data Structures
```rust
pub struct AdapterInfo {
    pub name: String,    // Adapter name
    pub status: String,  // Display name/status
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

## 10. How to Upload Installer to GitHub Releases Using GitHub CLI

1. **Login to GitHub CLI**
   ```bash
   gh auth login
   ```
   Follow the prompts to authenticate with your GitHub account.

2. **Create a Release and Upload the Installer**
   ```bash
   gh release create v1.0.0 "releases/Network_adapter_IP4_information_modification (Run_with_administrator_privileges).exe" --title "v1.0.0" --notes "First release version"
   ```
   - `v1.0.0` is the tag name (customizable)
   - `--title` is the release title
   - `--notes` is the release description

   Or upload to an existing release:
   ```bash
   gh release upload v1.0.0 "releases/Network_adapter_IP4_information_modification (Run_with_administrator_privileges).exe"
   ```

3. **Check the Result**
   After publishing, visit your repository’s Releases page to download the installer.

> Note: If the installer filename contains spaces, wrap the path in double quotes.

---

For more details on backend implementation, see `src-tauri/src/lib.rs` with full code comments.
