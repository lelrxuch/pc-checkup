//! Datos de ejemplo para `--demo`: sirven para capturas y para ver el
//! reporte sin depender del equipo donde se ejecuta.

use crate::report::*;

const GIB: u64 = 1024 * 1024 * 1024;

pub fn report() -> Report {
    Report {
        generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        machine: Machine {
            manufacturer: Some("Lenovo".into()),
            model: Some("IdeaPad 3 15IIL05".into()),
            serial: Some("PF2XXXXX".into()),
            board: Some("LENOVO LNVNB161216".into()),
            bios_version: Some("EMCN47WW".into()),
            bios_date: Some("2022-08-11".into()),
        },
        os: Os {
            name: Some("Windows 11 Home 23H2".into()),
            version: Some("10.0.22631".into()),
            kernel: None,
            hostname: Some("LAPTOP-ANA".into()),
            arch: "x86_64".into(),
            uptime_secs: 9 * 86_400 + 5 * 3600,
        },
        cpu: Cpu {
            name: "Intel(R) Core(TM) i3-1005G1 CPU @ 1.20GHz".into(),
            physical_cores: Some(2),
            logical_cores: 4,
            max_mhz: Some(1190),
        },
        memory: Memory {
            total_bytes: 7 * GIB + GIB * 7 / 10,
            slots: Some(2),
            modules: vec![MemoryModule {
                slot: Some("DIMM 0".into()),
                capacity_bytes: 8 * GIB,
                speed_mhz: Some(2666),
                kind: Some("DDR4".into()),
                manufacturer: Some("Samsung".into()),
                part_number: Some("M471A1K43DB1-CWE".into()),
            }],
        },
        disks: vec![
            PhysicalDisk {
                model: "WDC WD10SPZX-24Z10".into(),
                serial: Some("WD-WX12AXXXXXXX".into()),
                size_bytes: 1000 * 1000 * 1000 * 1000,
                media: Media::Hdd,
                bus: Some("SATA".into()),
                health: Health::Warning,
                temperature_c: Some(47),
                wear_pct: None,
                power_on_hours: Some(14_820),
                uncorrected_errors: Some(12),
            },
            PhysicalDisk {
                model: "Kingston SNV2S250G".into(),
                serial: None,
                size_bytes: 250 * 1000 * 1000 * 1000,
                media: Media::Ssd,
                bus: Some("NVMe".into()),
                health: Health::Ok,
                temperature_c: Some(41),
                wear_pct: Some(3),
                power_on_hours: Some(610),
                uncorrected_errors: Some(0),
            },
        ],
        volumes: vec![
            Volume {
                mount: "C:\\".into(),
                file_system: "NTFS".into(),
                total_bytes: 930 * GIB,
                available_bytes: 61 * GIB,
                media: Media::Hdd,
                removable: false,
                system: true,
            },
            Volume {
                mount: "D:\\".into(),
                file_system: "NTFS".into(),
                total_bytes: 232 * GIB,
                available_bytes: 201 * GIB,
                media: Media::Ssd,
                removable: false,
                system: false,
            },
        ],
        notes: vec![],
    }
}
