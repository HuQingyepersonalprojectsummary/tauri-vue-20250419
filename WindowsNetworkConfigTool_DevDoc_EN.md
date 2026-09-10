# Windows Network Configuration Tool — Developer Guide

Updated 2026-09-10 for the current 0.1.0 source and Windows x64 build. The [Chinese guide](./WindowsNetworkConfigTool_DevDoc_CN.md) covers the same implementation.

## 1. Environment and dependencies

This build was checked with Node.js 24.16.0, Rust/Cargo 1.91.1, PowerShell 7 and the Windows MSVC toolchain. Install Visual Studio C++ build tools and a Windows SDK. The desktop application requires Microsoft Edge WebView2 Runtime. These are observed build versions, not a claim that every earlier toolchain was tested.

The frontend lock resolves Vue 3.5.13, Vite 6.4.3, TypeScript 5.8.3, Tauri API 1.6.0 and Tauri CLI 1.6.3. Cargo.lock records exact Rust dependencies. Read lockfiles rather than assuming package.json range lower bounds are installed versions.

```powershell
npx --yes yarn@1.22.22 install --frozen-lockfile
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
npm run tauri -- dev
```

Use Yarn Classic for dependency installation; npm is used to run scripts. Do not introduce a second package-lock.json. Offline Cargo checks require a populated cache. Vite uses port 3000 and fails if the port is occupied. Running npm run dev alone provides a browser UI without native IPC.

## 2. Module boundaries

| Path | Responsibility |
|---|---|
| src/App.vue | Adapter selection, forms, DNS presets, history and status |
| src/composables/useNetworkConfig.ts | Current snapshot, per-adapter drafts, request sequencing, validation and apply |
| src/composables/useConfigHistory.ts | Versioned local history and independent storage warnings |
| src/services/networkClient.ts | Typed invoke wrappers |
| src/types/network.ts | Frontend DTOs |
| src/utils/validation.ts | IPv4, subnet, gateway and IPv6 validators |
| src-tauri/src/lib.rs | IPC registration, blocking task offload and write locks |
| src-tauri/src/domain.rs | Rust DTOs, validation and restoration verification |
| src-tauri/src/platform.rs | Trusted executables, process runner, snapshots, writes and compensation |
| src-tauri/src/main.rs | Desktop entry point and release GUI subsystem |
| src-tauri/tauri.conf.json | Product/version, window, security and installer configuration |
| tests/regression/ipv6 | Isolated production-code fault injection |
| scripts | Packaging and artifact export |
| releases | Portable executable, installers, checksums and source fingerprints |

Keep domain validation independent of OS IO. Add native operations in platform, route frontend calls through networkClient, and preserve separate network, recovery and storage failure states.

## 3. IPC contract

Rust serde uses camelCase. Rust and TypeScript DTOs are maintained manually: update both definitions, snapshot parsing, callers and fixtures when fields change.

| Command | invoke arguments | Result |
|---|---|---|
| get_network_adapters | none | AdapterInfo[] |
| get_current_config | `{ adapterName }` | AdapterSnapshot |
| apply_adapter_ipv4_config | `{ cfg }` | OperationResult |
| greet | `{ name }` | Legacy template greeting, unrelated to network configuration |

The apply command retains its historical IPv4 name but also handles IPv6, DNS and DoH.

### Configuration intent

- adapter selects the interface by name. Expected identity/version checking at edit time is not yet implemented.
- ipMode and dnsMode accept keep, dhcp or static. Missing values retain legacy inference from IPv4 fields; new callers should send explicit modes.
- Static IPv4 requires ip and mask. A non-empty gateway must be in the same subnet. DNS2 requires DNS1. Empty static IPv4 DNS restoration still has limitations.
- doh1 and doh2 describe the corresponding IPv4 DNS server: off/auto/manual, template and allowFallback. DoH server entries have system-wide scope.
- Missing ipv6Enabled preserves the binding. Explicit true/false changes it.
- ipv6Mode and ipv6DnsMode accept keep/dhcp/static; missing values mean keep. Do not infer missing history intent from an adapter snapshot.
- ipv6Ip, ipv6Prefix and ipv6Gateway describe a static address. Prefixes are 1–128; the backend defaults an omitted prefix to 64.
- Static ipv6DnsMode requires non-blank ipv6Dns1; ipv6Dns2 is optional. Empty, whitespace-only and secondary-only requests fail before any system write.

Example that only changes IPv6 DNS:

```typescript
await networkClient.applyAdapterIpv4Config({
  adapter: 'Ethernet',
  ipMode: 'keep', dnsMode: 'keep',
  ip: '', mask: '', gateway: '', dns1: '', dns2: '',
  ipv6Mode: 'keep', ipv6DnsMode: 'static',
  ipv6Dns1: '2001:db8::53', ipv6Dns2: ''
});
```

The address is for documentation; replace it with the intended resolver.

### Snapshots and outcomes

AdapterSnapshot contains identity, status, IPv4 addresses/gateways/DNS, DoH and IPv6 state. Each IPv6 address includes ipAddress, prefixLength, prefixOrigin and suffixOrigin. Missing origin stays unknown; it must never be inferred as Manual. When an adapter has IPv6 unbound or disabled (e.g., `ms_tcpip6` is False), CIM queries (`Get-NetIPInterface -AddressFamily IPv6` and `Get-DnsClientServerAddress`) throw "No matching objects". The script catches this and performs a graceful fallback (`ipv6Enabled=false`, empty IPv6 lists, `ipv6DhcpEnabled=true`), ensuring IPv4 configuration queries succeed uninterrupted.

OperationResult.success means the requested changes passed read-back verification. message preserves the outcome or initial error. rolledBack means compensation commands and restoration verification both passed, not merely that rollback was attempted. rollbackMessage must be shown on failure. snapshot may be null if the final state could not be read; clear stale UI state in that case.

Preflight failures can reject the IPC Promise. Failures after writes generally return success=false and a recovery diagnosis. Handle both paths.

## 4. Transaction and recovery

1. IPC offloads OS work to spawn_blocking and serializes writes with an in-process lock and Windows named mutex.
2. Validate inputs, obtain the original snapshot and check adapter state, DoH capability and IPv6 address origins before writes.
3. Apply only the explicitly requested IPv4, extension and IPv6 modes. keep skips the corresponding write.
4. Record every attempted IPv6 address deletion/addition before execution. Combine this journal with the immutable original snapshot to retain the original prefix and origin. Timeouts may represent partially completed writes.
5. Read back and compare the result, including normalized IPv6 addresses and complete ordered DNS lists.
6. On failure, clean up touched IPv6 addresses, including replacements of the same IP. Restore original manual addresses with their original prefix, even on an otherwise automatic interface. Let DHCP/SLAAC recreate automatic addresses rather than adding manual substitutes.
7. Verify restoration. Cleanup failures, manual residue, changed manual prefixes or unknown origins prevent rolledBack=true. Automatic address churn is allowed.

This is command-by-command compensation, not an atomic OS transaction or a guarantee of uninterrupted connectivity. The transaction journal is in memory and cannot resume after a crash. Route properties, address lifetimes and complex multi-address/gateway/DNS restoration remain documented limitations.

## 5. Processes, permissions and state

PowerShell receives JSON through stdin and uses fixed scripts; netsh receives separate arguments. Resolve executables from trusted system directories. The runner applies CREATE_NO_WINDOW, timeouts and Windows Job Object management. Snapshot CIM queries (`Get-NetIPInterface`, `Get-DnsClientServerAddress`) employ `-ErrorAction Stop` and regex fallback matching against absent interface instances (e.g. disabled IPv6 stacks) to ensure robust enumeration. Job creation/assignment failures and termination confirmation still require work. A Global mutex timeout does not fall back to Local, while the access-denied fallback remains an isolation limitation.

There is no on-demand elevated helper or requireAdministrator application manifest. Users must run the application as administrator to write network settings. An installer's UAC request does not establish that the application elevates automatically.

Snapshots and form state are separate. Request sequencing suppresses stale query responses. Per-adapter drafts live in memory for the current run. History is stored under net_config_history_v1 in WebView localStorage, uses schemaVersion 1 and retains at most 10 records. Invalid entries are filtered; persistence failures do not change the network operation result. History is not a full system backup.

## 6. Verification and release

```powershell
npm run typecheck
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --offline --all-targets -- -D warnings
npm run test:regression
npm run release
```

Regression probes execute current production transaction logic, actual Vue composables and extracted PowerShell scripts with controlled IO. Results go to the ignored tests/regression/ipv6/output directory; any failed probe/assertion exits nonzero. Keep extraction boundaries synchronized when production functions move.

The release script builds frontend resources, the executable, MSI and NSIS, verifies artifact freshness, then exports artifacts and hashes. See the [packaging guide](./打包说明.md). The current release is unsigned, x64, with a WebView2 download-bootstrapper installer policy. Native UI, real network changes, UAC and installation/uninstallation still require acceptance testing.

Synchronize package.json, Cargo.toml, the application entry in Cargo.lock and tauri.conf.json when changing the version. Update the applicable lockfile when changing dependencies. Before pushing, verify docs, source fingerprints and artifact checksums, then use a normal fast-forward push.

Repeated dated audit reports and raw logs have been consolidated into [verification](./docs/verification.md) and [known limitations](./docs/known-limitations.md). Reusable probes now live under tests. Removing historical reports does not close unresolved findings.
