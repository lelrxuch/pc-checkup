//! Detalle de hardware en Linux: DMI desde `/sys`, discos desde `/sys/block`
//! y salud SMART con `smartctl` cuando está instalado y hay permisos.

use super::{PlatformData, codes};
use crate::report::{Health, Machine, Media, PhysicalDisk};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::process::Command;

const DMI: &str = "/sys/class/dmi/id";
const BLOCK: &str = "/sys/block";

pub(crate) fn collect() -> PlatformData {
    let mut data = PlatformData {
        machine: machine(),
        ..Default::default()
    };
    if data.machine.serial.is_none() && Path::new(DMI).join("product_serial").exists() {
        data.notes
            .push("El número de serie solo se puede leer como root.".into());
    }
    data.notes
        .push("El detalle de los módulos de RAM requiere dmidecode como root.".into());

    data.disks = disks(&mut data.notes);
    data
}

fn read(path: impl AsRef<Path>) -> Option<String> {
    codes::clean(fs::read_to_string(path).ok())
}

fn dmi(field: &str) -> Option<String> {
    read(Path::new(DMI).join(field))
}

fn machine() -> Machine {
    let board = [dmi("board_vendor"), dmi("board_name")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Machine {
        manufacturer: dmi("sys_vendor"),
        model: dmi("product_name"),
        serial: dmi("product_serial"),
        board: (!board.is_empty()).then(|| board.join(" ")),
        bios_version: dmi("bios_version"),
        bios_date: dmi("bios_date").map(|d| us_date_to_iso(&d).unwrap_or(d)),
    }
}

/// El kernel expone `bios_date` como `MM/DD/YYYY`.
fn us_date_to_iso(d: &str) -> Option<String> {
    let mut it = d.split('/');
    let (m, day, y) = (it.next()?, it.next()?, it.next()?);
    if y.len() == 4 && m.len() == 2 && day.len() == 2 {
        Some(format!("{y}-{m}-{day}"))
    } else {
        None
    }
}

/// Dispositivos de bloque que no son discos físicos.
fn is_virtual_block(name: &str) -> bool {
    ["loop", "ram", "zram", "dm-", "md", "sr", "nbd", "fd"]
        .iter()
        .any(|p| name.starts_with(p))
}

fn disks(notes: &mut Vec<String>) -> Vec<PhysicalDisk> {
    let Ok(entries) = fs::read_dir(BLOCK) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !is_virtual_block(n))
        .collect();
    names.sort();

    let has_smartctl = Command::new("smartctl").arg("--version").output().is_ok();
    let mut smart_denied = false;

    let disks: Vec<PhysicalDisk> = names
        .iter()
        .filter_map(|name| {
            let dir = Path::new(BLOCK).join(name);
            let sectors: u64 = read(dir.join("size"))?.parse().ok()?;
            if sectors == 0 {
                return None;
            }
            let mut disk = PhysicalDisk {
                model: read(dir.join("device/model")).unwrap_or_else(|| name.clone()),
                serial: read(dir.join("device/serial")),
                size_bytes: sectors * 512,
                media: match fs::read_to_string(dir.join("queue/rotational")).as_deref() {
                    Ok("1\n") => Media::Hdd,
                    Ok("0\n") => Media::Ssd,
                    _ => Media::Unknown,
                },
                bus: bus_from_name(name).map(str::to_string),
                health: Health::Unknown,
                temperature_c: None,
                wear_pct: None,
                power_on_hours: None,
                uncorrected_errors: None,
            };
            if has_smartctl {
                match smartctl(name) {
                    Some(s) => s.apply(&mut disk),
                    None => smart_denied = true,
                }
            }
            Some(disk)
        })
        .collect();

    if !has_smartctl {
        notes.push(
            "Instala smartmontools y ejecuta con sudo para ver la salud SMART de los discos."
                .into(),
        );
    } else if smart_denied {
        notes.push("Ejecuta con sudo para ver la salud SMART de los discos.".into());
    }
    disks
}

fn bus_from_name(name: &str) -> Option<&'static str> {
    if name.starts_with("nvme") {
        Some("NVMe")
    } else if name.starts_with("mmcblk") {
        Some("MMC")
    } else {
        None
    }
}

fn smartctl(name: &str) -> Option<Smart> {
    let out = Command::new("smartctl")
        .args(["-j", "-H", "-A", "-i"])
        .arg(format!("/dev/{name}"))
        .output()
        .ok()?;
    Smart::parse(&out.stdout)
}

/// Subconjunto de la salida JSON de `smartctl -j`.
#[derive(Debug, Default, Deserialize)]
struct Smart {
    smart_status: Option<SmartStatus>,
    temperature: Option<Current>,
    power_on_time: Option<PowerOn>,
    nvme_smart_health_information_log: Option<NvmeLog>,
    ata_smart_attributes: Option<AtaAttributes>,
}

#[derive(Debug, Deserialize)]
struct SmartStatus {
    passed: bool,
}

#[derive(Debug, Deserialize)]
struct Current {
    current: u32,
}

#[derive(Debug, Deserialize)]
struct PowerOn {
    hours: u64,
}

#[derive(Debug, Deserialize)]
struct NvmeLog {
    percentage_used: Option<u32>,
    media_errors: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AtaAttributes {
    table: Vec<AtaAttribute>,
}

#[derive(Debug, Deserialize)]
struct AtaAttribute {
    id: u32,
    raw: AtaRaw,
}

#[derive(Debug, Deserialize)]
struct AtaRaw {
    value: u64,
}

/// Sectores reasignados, pendientes e incorregibles.
const ATA_BAD_SECTOR_IDS: [u32; 3] = [5, 197, 198];

impl Smart {
    /// `None` si smartctl no pudo leer el disco (normalmente, falta de permisos).
    fn parse(json: &[u8]) -> Option<Self> {
        let s: Smart = serde_json::from_slice(json).ok()?;
        s.smart_status.is_some().then_some(s)
    }

    fn apply(&self, disk: &mut PhysicalDisk) {
        disk.health = match &self.smart_status {
            Some(SmartStatus { passed: true }) => Health::Ok,
            Some(SmartStatus { passed: false }) => Health::Failing,
            None => Health::Unknown,
        };
        disk.temperature_c = self.temperature.as_ref().map(|t| t.current);
        disk.power_on_hours = self.power_on_time.as_ref().map(|p| p.hours);
        if let Some(nvme) = &self.nvme_smart_health_information_log {
            disk.wear_pct = nvme.percentage_used;
            disk.uncorrected_errors = nvme.media_errors;
        }
        if let Some(ata) = &self.ata_smart_attributes {
            let bad: u64 = ata
                .table
                .iter()
                .filter(|a| ATA_BAD_SECTOR_IDS.contains(&a.id))
                .map(|a| a.raw.value)
                .sum();
            disk.uncorrected_errors = Some(bad);
        }
        if disk.health == Health::Ok && disk.uncorrected_errors.is_some_and(|n| n > 0) {
            disk.health = Health::Warning;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_disk() -> PhysicalDisk {
        PhysicalDisk {
            model: "x".into(),
            serial: None,
            size_bytes: 0,
            media: Media::Ssd,
            bus: None,
            health: Health::Unknown,
            temperature_c: None,
            wear_pct: None,
            power_on_hours: None,
            uncorrected_errors: None,
        }
    }

    #[test]
    fn parses_nvme_smart() {
        let json = br#"{
            "smart_status": {"passed": true},
            "temperature": {"current": 38},
            "power_on_time": {"hours": 4120},
            "nvme_smart_health_information_log": {"percentage_used": 7, "media_errors": 0}
        }"#;
        let mut d = blank_disk();
        Smart::parse(json).unwrap().apply(&mut d);
        assert_eq!(d.health, Health::Ok);
        assert_eq!(d.temperature_c, Some(38));
        assert_eq!(d.power_on_hours, Some(4120));
        assert_eq!(d.wear_pct, Some(7));
        assert_eq!(d.uncorrected_errors, Some(0));
    }

    #[test]
    fn ata_bad_sectors_downgrade_health() {
        let json = br#"{
            "smart_status": {"passed": true},
            "ata_smart_attributes": {"table": [
                {"id": 5, "raw": {"value": 8}},
                {"id": 9, "raw": {"value": 20000}},
                {"id": 197, "raw": {"value": 2}}
            ]}
        }"#;
        let mut d = blank_disk();
        Smart::parse(json).unwrap().apply(&mut d);
        assert_eq!(d.uncorrected_errors, Some(10));
        assert_eq!(d.health, Health::Warning);
    }

    #[test]
    fn permission_error_yields_none() {
        let json =
            br#"{"smartctl": {"exit_status": 2, "messages": [{"string": "Permission denied"}]}}"#;
        assert!(Smart::parse(json).is_none());
    }

    #[test]
    fn helpers() {
        assert_eq!(us_date_to_iso("03/15/2023").as_deref(), Some("2023-03-15"));
        assert_eq!(us_date_to_iso("2023"), None);
        assert!(is_virtual_block("loop0"));
        assert!(is_virtual_block("zram0"));
        assert!(!is_virtual_block("nvme0n1"));
        assert!(!is_virtual_block("sda"));
        assert_eq!(bus_from_name("nvme0n1"), Some("NVMe"));
    }
}
