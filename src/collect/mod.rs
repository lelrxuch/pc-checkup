//! Recolección de datos. Lo común (CPU, RAM total, volúmenes, SO) sale de
//! `sysinfo`; el detalle de placa, módulos de RAM y salud de discos depende
//! de la plataforma.

pub mod codes;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use crate::report::{Cpu, Machine, Media, Memory, Os, PhysicalDisk, Report, Volume};
use sysinfo::{DiskKind, Disks, System};

/// Lo que aporta cada plataforma además de la parte común.
#[derive(Default)]
pub(crate) struct PlatformData {
    pub machine: Machine,
    pub memory_slots: Option<u32>,
    pub memory_modules: Vec<crate::report::MemoryModule>,
    pub disks: Vec<PhysicalDisk>,
    pub notes: Vec<String>,
}

pub fn collect() -> Report {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let platform = platform_data();

    Report {
        generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        machine: platform.machine,
        os: Os {
            name: System::long_os_version().or_else(System::name),
            version: System::os_version(),
            kernel: System::kernel_version(),
            hostname: System::host_name(),
            arch: System::cpu_arch(),
            uptime_secs: System::uptime(),
        },
        cpu: cpu(&sys),
        memory: Memory {
            total_bytes: sys.total_memory(),
            slots: platform.memory_slots,
            modules: platform.memory_modules,
        },
        disks: platform.disks,
        volumes: volumes(),
        notes: platform.notes,
    }
}

fn cpu(sys: &System) -> Cpu {
    let cpus = sys.cpus();
    let first = cpus.first();
    Cpu {
        name: first
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Desconocido".into()),
        physical_cores: System::physical_core_count(),
        logical_cores: cpus.len(),
        max_mhz: cpus.iter().map(|c| c.frequency()).max().filter(|&f| f > 0),
    }
}

fn volumes() -> Vec<Volume> {
    let disks = Disks::new_with_refreshed_list();
    let system_mount = system_mount();
    let mut out: Vec<Volume> = disks
        .list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .filter(|d| !is_pseudo_fs(&d.file_system().to_string_lossy()))
        .map(|d| {
            let mount = d.mount_point().to_string_lossy().into_owned();
            Volume {
                system: mount.eq_ignore_ascii_case(&system_mount),
                mount,
                file_system: d.file_system().to_string_lossy().into_owned(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
                media: match d.kind() {
                    DiskKind::SSD => Media::Ssd,
                    DiskKind::HDD => Media::Hdd,
                    _ => Media::Unknown,
                },
                removable: d.is_removable(),
            }
        })
        .collect();
    dedupe_volumes(&mut out);
    out.sort_by(|a, b| b.system.cmp(&a.system).then_with(|| a.mount.cmp(&b.mount)));
    out
}

/// En Linux un mismo sistema de archivos (btrfs con subvolúmenes, bind mounts)
/// aparece montado varias veces con el mismo tamaño. Se deja una sola entrada,
/// priorizando la del sistema y luego la ruta más corta.
fn dedupe_volumes(vols: &mut Vec<Volume>) {
    vols.sort_by(|a, b| {
        b.system
            .cmp(&a.system)
            .then_with(|| a.mount.len().cmp(&b.mount.len()))
    });
    let mut seen = Vec::new();
    vols.retain(|v| {
        let key = (v.total_bytes, v.available_bytes, v.file_system.clone());
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
}

fn is_pseudo_fs(fs: &str) -> bool {
    matches!(
        fs,
        "tmpfs" | "devtmpfs" | "overlay" | "squashfs" | "efivarfs" | "ramfs" | "fuse.portal"
    ) || fs.starts_with("fuse.")
}

#[cfg(windows)]
fn system_mount() -> String {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    format!("{}\\", drive.trim_end_matches('\\'))
}

#[cfg(not(windows))]
fn system_mount() -> String {
    "/".into()
}

#[cfg(windows)]
fn platform_data() -> PlatformData {
    windows::collect()
}

#[cfg(target_os = "linux")]
fn platform_data() -> PlatformData {
    linux::collect()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_data() -> PlatformData {
    PlatformData {
        notes: vec!["Plataforma sin soporte para detalle de hardware.".into()],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(mount: &str, total: u64, avail: u64, system: bool) -> Volume {
        Volume {
            mount: mount.into(),
            file_system: "btrfs".into(),
            total_bytes: total,
            available_bytes: avail,
            media: Media::Ssd,
            removable: false,
            system,
        }
    }

    #[test]
    fn dedupe_keeps_system_mount_of_shared_fs() {
        let mut v = vec![
            vol("/home", 500, 100, false),
            vol("/", 500, 100, true),
            vol("/var", 500, 100, false),
            vol("/mnt/datos", 1000, 900, false),
        ];
        dedupe_volumes(&mut v);
        let mounts: Vec<_> = v.iter().map(|x| x.mount.as_str()).collect();
        assert_eq!(mounts, ["/", "/mnt/datos"]);
    }

    #[test]
    fn pseudo_filesystems_are_skipped() {
        assert!(is_pseudo_fs("tmpfs"));
        assert!(is_pseudo_fs("fuse.gvfsd-fuse"));
        assert!(!is_pseudo_fs("ext4"));
        assert!(!is_pseudo_fs("NTFS"));
    }
}
