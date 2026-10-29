//! Environment check (plan T34): GPU, RAM, Ollama, winget and Claude Code, plus the model
//! tier recommendation. The checks run in parallel, each with a timeout of at most 3 s, and
//! none of them fails: a problem means "not available".

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;
use ts_rs::TS;

use super::model_tiers::{Recommendation, recommend};
use crate::settings::Settings;
use crate::translation::providers::{ClaudeStatus, claude_code};

const CHECK_TIMEOUT: Duration = Duration::from_secs(3);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const VENDOR_NVIDIA: u32 = 0x10DE;
const VENDOR_AMD: u32 = 0x1002;
const VENDOR_INTEL: u32 = 0x8086;
/// A dedicated GPU needs at least this much VRAM to be worth using (a "4 GB" card reports a bit
/// less than 4096).
const USABLE_MIN_VRAM_MB: u64 = 3_900;
/// AMD APUs report less than this as dedicated memory.
const AMD_INTEGRATED_MAX_VRAM_MB: u64 = 1_024;
const IGPU_NAME_MARKERS: [&str; 4] = ["radeon graphics", "radeon(tm) graphics", "uhd", "iris"];

/// One graphics adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GpuInfo {
    pub name: String,
    /// Dedicated video memory in MB.
    #[ts(type = "number")]
    pub vram_mb: u64,
    /// Integrated graphics (shares system memory).
    pub integrated: bool,
    /// Counted as a GPU for the model tier: a discrete NVIDIA or AMD card with enough VRAM.
    /// This is only a guess; the benchmark's GPU offload corrects it.
    pub usable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OllamaEnv {
    pub installed: bool,
    pub running: bool,
    pub version: Option<String>,
    pub models: Vec<String>,
}

/// What `environment_check` returns.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EnvironmentReport {
    pub os: String,
    /// Total system RAM in MB.
    #[ts(type = "number")]
    pub ram_mb: u64,
    /// Available system RAM in MB.
    #[ts(type = "number")]
    pub ram_available_mb: u64,
    /// True when at least one GPU is usable (the recommendation is not `none`).
    pub usable_gpu: bool,
    pub gpus: Vec<GpuInfo>,
    pub recommendation: Recommendation,
    pub winget: bool,
    pub ollama: OllamaEnv,
    pub claude: ClaudeStatus,
}

/// Decides `(integrated, usable)` for an adapter from its vendor ID, name and dedicated VRAM.
/// The usable-GPU rule lives here only.
pub fn classify_gpu(vendor_id: u32, name: &str, vram_mb: u64) -> (bool, bool) {
    let lower = name.to_lowercase();
    let integrated = vendor_id == VENDOR_INTEL
        || (vendor_id == VENDOR_AMD && vram_mb < AMD_INTEGRATED_MAX_VRAM_MB)
        || IGPU_NAME_MARKERS.iter().any(|m| lower.contains(m));
    let usable = !integrated
        && (vendor_id == VENDOR_NVIDIA || vendor_id == VENDOR_AMD)
        && vram_mb >= USABLE_MIN_VRAM_MB;
    (integrated, usable)
}

/// Runs every check and builds the report.
pub async fn check(settings: &Settings) -> EnvironmentReport {
    let (gpus, ram, winget, ollama, claude) = tokio::join!(
        within(list_gpus()),
        within(system_ram()),
        winget_available(),
        ollama_env(settings),
        claude_code::status(settings),
    );
    let gpus = gpus.unwrap_or_default();
    let (ram_mb, ram_available_mb) = ram.unwrap_or((0, 0));
    let recommendation = recommend(&gpus);
    EnvironmentReport {
        os: os_name(),
        ram_mb,
        ram_available_mb,
        usable_gpu: gpus.iter().any(|g| g.usable),
        gpus,
        recommendation,
        winget,
        ollama,
        claude,
    }
}

async fn within<T>(fut: impl std::future::Future<Output = T>) -> Option<T> {
    timeout(CHECK_TIMEOUT, fut).await.ok()
}

// ---- GPU and RAM ----

async fn list_gpus() -> Vec<GpuInfo> {
    tokio::task::spawn_blocking(enumerate_gpus)
        .await
        .unwrap_or_default()
}

#[cfg(windows)]
fn enumerate_gpus() -> Vec<GpuInfo> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
    };

    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(f) => f,
        Err(e) => {
            log::info!("DXGI factory unavailable: {e}");
            return Vec::new();
        }
    };
    let mut gpus = Vec::new();
    // `EnumAdapters1` fails with DXGI_ERROR_NOT_FOUND after the last adapter.
    for index in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
            continue;
        };
        if desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
            continue;
        }
        let len = desc
            .Description
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(desc.Description.len());
        let name = String::from_utf16_lossy(&desc.Description[..len]);
        let vram_mb = (desc.DedicatedVideoMemory as u64) / (1024 * 1024);
        let (integrated, usable) = classify_gpu(desc.VendorId, &name, vram_mb);
        gpus.push(GpuInfo {
            name,
            vram_mb,
            integrated,
            usable,
        });
    }
    gpus
}

#[cfg(not(windows))]
fn enumerate_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

/// `(total, available)` system RAM in MB.
async fn system_ram() -> (u64, u64) {
    tokio::task::spawn_blocking(ram_mb).await.unwrap_or((0, 0))
}

#[cfg(windows)]
fn ram_mb() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return (0, 0);
    }
    (
        status.ullTotalPhys / (1024 * 1024),
        status.ullAvailPhys / (1024 * 1024),
    )
}

#[cfg(not(windows))]
fn ram_mb() -> (u64, u64) {
    (0, 0)
}

/// "Windows 11 (build 26200)" style description; plain "Windows" if the version is unknown.
#[cfg(windows)]
fn os_name() -> String {
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(info: *mut OSVERSIONINFOW) -> i32;
    }

    let mut info: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    if unsafe { RtlGetVersion(&mut info) } != 0 {
        return "Windows".into();
    }
    // Windows 11 still reports major version 10; it starts at build 22000.
    let name = if info.dwMajorVersion == 10 && info.dwBuildNumber >= 22_000 {
        "Windows 11".to_owned()
    } else {
        format!("Windows {}", info.dwMajorVersion)
    };
    format!("{name} (build {})", info.dwBuildNumber)
}

#[cfg(not(windows))]
fn os_name() -> String {
    std::env::consts::OS.to_owned()
}

// ---- winget ----

/// winget is available when `winget --version` runs and succeeds.
async fn winget_available() -> bool {
    let mut cmd = Command::new("winget");
    cmd.arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    match timeout(CHECK_TIMEOUT, cmd.status()).await {
        Ok(Ok(status)) => status.success(),
        _ => false,
    }
}

// ---- Ollama ----

async fn ollama_env(settings: &Settings) -> OllamaEnv {
    let base = settings
        .ollama
        .endpoint
        .trim()
        .trim_end_matches('/')
        .to_owned();
    let client = reqwest::Client::new();
    let (installed, models, version) = tokio::join!(
        async { ollama_installed() },
        get_json(&client, format!("{base}/api/tags")),
        get_json(&client, format!("{base}/api/version")),
    );
    let running = models.is_some();
    let models = models
        .as_ref()
        .and_then(|b| b.get("models"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|m| m.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let version = version
        .as_ref()
        .and_then(|b| b.get("version"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    OllamaEnv {
        // A server that answers is installed, wherever its binary lives.
        installed: installed || running,
        running,
        version,
        models,
    }
}

async fn get_json(client: &reqwest::Client, url: String) -> Option<Value> {
    client
        .get(url)
        .timeout(CHECK_TIMEOUT)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()
}

/// `ollama.exe` on `PATH` or in `%LOCALAPPDATA%\Programs\Ollama`.
fn ollama_installed() -> bool {
    let default = std::env::var_os("LOCALAPPDATA").map(|d| {
        PathBuf::from(d)
            .join("Programs")
            .join("Ollama")
            .join("ollama.exe")
    });
    if default.is_some_and(|p| p.is_file()) {
        return true;
    }
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| dir.join("ollama.exe").is_file())
    })
}
