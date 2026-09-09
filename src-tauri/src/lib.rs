pub mod domain;
pub mod platform;

use domain::{AdapterInfo, AdapterSnapshot, Ipv4Config, OperationResult};
use tauri::async_runtime::Mutex;
use tauri::State;

/// 全局网络配置写操作锁，确保当前进程内写操作串行化执行 (A-03, A-04)
pub struct NetworkLock(pub Mutex<()>);

#[cfg(windows)]
mod sys_mutex {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    type RawHandle = *mut std::ffi::c_void;
    const WAIT_OBJECT_0: u32 = 0x00000000;
    const WAIT_ABANDONED: u32 = 0x00000080;
    const WAIT_TIMEOUT: u32 = 0x00000102;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(
            lpMutexAttributes: *mut std::ffi::c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> RawHandle;
        fn WaitForSingleObject(hHandle: RawHandle, dwMilliseconds: u32) -> u32;
        fn ReleaseMutex(hMutex: RawHandle) -> i32;
        fn CloseHandle(hObject: RawHandle) -> i32;
    }

    /// 跨进程具名互斥体保护，防止多个应用实例交错修改系统网络 (R-08, F-07)
    pub struct CrossProcessLock {
        handle: RawHandle,
    }

    impl CrossProcessLock {
        pub fn acquire(name: &str, timeout_ms: u32) -> Result<Self, String> {
            let wide: Vec<u16> = OsStr::new(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe { CreateMutexW(null_mut(), 0, wide.as_ptr()) };
            if handle.is_null() {
                return Err("无法创建或打开系统全局命名互斥体".to_string());
            }
            let res = unsafe { WaitForSingleObject(handle, timeout_ms) };
            if res == WAIT_OBJECT_0 || res == WAIT_ABANDONED {
                Ok(CrossProcessLock { handle })
            } else if res == WAIT_TIMEOUT {
                unsafe { CloseHandle(handle) };
                Err("系统检测到另一个网络配置进程正在执行，请稍候再试 (跨进程互斥保护)".to_string())
            } else {
                unsafe { CloseHandle(handle) };
                Err("获取系统配置互斥锁失败".to_string())
            }
        }

        /// 优先尝试跨会话 Global 互斥体，若因权限受限无法创建则优雅回退至 Local 互斥体 (F-07)
        pub fn acquire_global_or_local(base_name: &str, timeout_ms: u32) -> Result<Self, String> {
            let global_name = format!("Global\\{}", base_name);
            if let Ok(lock) = Self::acquire(&global_name, timeout_ms) {
                return Ok(lock);
            }
            let local_name = format!("Local\\{}", base_name);
            Self::acquire(&local_name, timeout_ms)
        }
    }

    impl Drop for CrossProcessLock {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    ReleaseMutex(self.handle);
                    CloseHandle(self.handle);
                }
            }
        }
    }
}

pub mod commands {
    use super::*;

    #[tauri::command]
    pub fn greet(name: &str) -> String {
        format!("Hello, {}! You've been greeted from Rust!", name)
    }

    /// 异步获取所有网络适配器列表，避免阻塞 UI 线程 (A-04)
    #[tauri::command]
    pub async fn get_network_adapters() -> Result<Vec<AdapterInfo>, String> {
        tauri::async_runtime::spawn_blocking(platform::list_network_adapters)
            .await
            .map_err(|e| format!("调度查询适配器任务失败: {}", e))?
    }

    /// 异步单次提取适配器完整网络快照 (A-04, A-05)
    #[tauri::command]
    pub async fn get_current_config(adapter_name: String) -> Result<AdapterSnapshot, String> {
        tauri::async_runtime::spawn_blocking(move || platform::get_adapter_snapshot(&adapter_name))
            .await
            .map_err(|e| format!("调度获取当前配置任务失败: {}", e))?
    }

    /// 事务式应用 IPv4 配置，包含事前快照、校验、跨进程互斥锁与自动回滚 (A-01, A-03, A-06, R-08, F-07)
    #[tauri::command]
    pub async fn apply_adapter_ipv4_config(
        cfg: Ipv4Config,
        lock: State<'_, NetworkLock>,
    ) -> Result<OperationResult, String> {
        let _guard = lock.0.lock().await;
        tauri::async_runtime::spawn_blocking(move || {
            #[cfg(windows)]
            let _sys_guard = sys_mutex::CrossProcessLock::acquire_global_or_local(
                "WindowsNetworkConfigTool_WriteLock",
                5000,
            )?;

            platform::apply_adapter_ipv4_config_transactional(&cfg)
        })
        .await
        .map_err(|e| format!("调度应用配置任务失败: {}", e))?
    }
}

/// 启动 Tauri 应用程序
pub fn run() {
    tauri::Builder::default()
        .manage(NetworkLock(Mutex::new(())))
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_network_adapters,
            commands::apply_adapter_ipv4_config,
            commands::get_current_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
