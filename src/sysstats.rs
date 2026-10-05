//! Host system metrics for the dashboard.

use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Default, Serialize)]
pub struct SystemStats {
    pub cpu_percent: f64,
    pub cpu_count: usize,
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub uptime_sec: u64,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    pub load_avg: [f64; 3],
    pub data_dir_bytes: u64,
}

/// Gather a fresh snapshot. `data_dir` selects which mount/disk to report.
pub async fn collect(data_dir: &std::path::Path) -> SystemStats {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    // CPU% needs two samples; take one now (first call shows coarse values).
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    sys.refresh_cpu_all();

    let cpu_percent = sys
        .cpus()
        .iter()
        .map(|c| c.cpu_usage() as f64)
        .sum::<f64>()
        / sys.cpus().len().max(1) as f64;

    let disks = Disks::new_with_refreshed_list();
    // Find the disk that best (longest-prefix) covers the data dir.
    let (mut dtotal, mut dused) = (0u64, 0u64);
    let mut best = 0usize;
    for d in disks.list() {
        let mp = d.mount_point();
        if data_dir.starts_with(mp) && mp.as_os_str().len() >= best {
            best = mp.as_os_str().len();
            dtotal = d.total_space();
            dused = d.total_space().saturating_sub(d.available_space());
        }
    }

    let load = System::load_average();
    SystemStats {
        cpu_percent,
        cpu_count: sys.cpus().len(),
        mem_total: sys.total_memory(),
        mem_used: sys.used_memory(),
        mem_available: sys.available_memory(),
        swap_total: sys.total_swap(),
        swap_used: sys.used_swap(),
        disk_total: dtotal,
        disk_used: dused,
        uptime_sec: System::uptime(),
        hostname: System::host_name().unwrap_or_default(),
        os: System::long_os_version().unwrap_or_default(),
        kernel: System::kernel_version().unwrap_or_default(),
        arch: std::env::consts::ARCH.to_string(),
        load_avg: [load.one, load.five, load.fifteen],
        data_dir_bytes: crate::files::dir_size(data_dir).await,
    }
}

/// Format bytes human-readably (KiB, MiB, ...).
pub fn human_bytes(b: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// Format seconds as "3d 4h 5m".
pub fn human_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = secs % 86400 / 3600;
    let m = secs % 3600 / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m {}s", secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_format() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KiB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn uptime_format() {
        assert_eq!(human_uptime(30), "0m 30s");
        assert_eq!(human_uptime(3600 + 120), "1h 2m");
        assert_eq!(human_uptime(2 * 86400 + 3600), "2d 1h 0m");
    }

    #[tokio::test]
    async fn collect_works() {
        let s = collect(std::path::Path::new("/")).await;
        assert!(s.mem_total > 0);
        assert!(s.cpu_count > 0);
    }
}
