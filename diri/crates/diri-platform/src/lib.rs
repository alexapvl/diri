//! Native OS mechanisms shared by the Engine, Holder and its clients.
//! No product state, terminal parsing or orchestration lives here.
pub mod ipc;
pub mod launch;
pub mod pipe;
pub mod poll;
pub mod security;
pub mod signals;

#[cfg(windows)]
pub use windows_sys;

pub fn home_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let value = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let value = std::env::var_os("HOME");
    value
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
}

pub fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.into()
    }
}

pub mod directory;

#[cfg(windows)]
pub mod process;

#[cfg(windows)]
pub mod job;

pub mod child;

pub fn curl_executable() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        std::path::PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
        )
        .join("System32/curl.exe")
    }
    #[cfg(unix)]
    {
        "/usr/bin/curl".into()
    }
}
