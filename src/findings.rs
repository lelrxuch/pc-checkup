//! Reglas que convierten los datos en hallazgos con semáforo. Los umbrales
//! son conservadores: un hallazgo rojo debe justificar una acción inmediata.

use crate::format::bytes;
use crate::report::{Health, Media, PhysicalDisk, Report, Volume};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub level: Level,
    pub area: &'static str,
    pub title: String,
    pub detail: String,
    pub action: String,
}

const GIB: u64 = 1024 * 1024 * 1024;
const MIN_CHECKED_VOLUME: u64 = 4 * GIB;

pub fn analyze(r: &Report) -> Vec<Finding> {
    let mut out = Vec::new();
    for d in &r.disks {
        disk(d, &mut out);
    }
    // Particiones pequeñas (EFI, /boot, recuperación) están casi llenas por diseño.
    for v in r
        .volumes
        .iter()
        .filter(|v| !v.removable && (v.system || v.total_bytes >= MIN_CHECKED_VOLUME))
    {
        volume(v, &mut out);
    }
    memory(r, &mut out);
    out.sort_by_key(|f| f.level);
    out
}

fn push(
    out: &mut Vec<Finding>,
    level: Level,
    area: &'static str,
    title: impl Into<String>,
    detail: impl Into<String>,
    action: impl Into<String>,
) {
    out.push(Finding {
        level,
        area,
        title: title.into(),
        detail: detail.into(),
        action: action.into(),
    });
}

fn disk(d: &PhysicalDisk, out: &mut Vec<Finding>) {
    let name = &d.model;
    match d.health {
        Health::Failing => push(
            out,
            Level::Critical,
            "Disco",
            format!("{name}: el disco reporta falla"),
            "El propio disco indica que su estado SMART no es saludable.",
            "Respaldar la información hoy y reemplazar el disco.",
        ),
        // Si la degradación se debe a sectores ilegibles, ya lo cubre el hallazgo crítico.
        Health::Warning if !d.uncorrected_errors.is_some_and(|n| n > 0) => push(
            out,
            Level::Warning,
            "Disco",
            format!("{name}: el disco reporta degradación"),
            "El estado de salud no es óptimo; puede fallar en el corto plazo.",
            "Respaldar la información y planear el reemplazo.",
        ),
        Health::Warning | Health::Ok | Health::Unknown => {}
    }

    if let Some(n) = d.uncorrected_errors.filter(|&n| n > 0) {
        push(
            out,
            Level::Critical,
            "Disco",
            format!("{name}: {n} errores de lectura no corregibles"),
            "Hay sectores que el disco ya no puede leer de forma confiable.",
            "Respaldar la información y reemplazar el disco.",
        );
    }

    if let Some(w) = d.wear_pct {
        if w >= 90 {
            push(
                out,
                Level::Critical,
                "Disco",
                format!("{name}: {w}% de vida útil consumida"),
                "El SSD está al final de la escritura garantizada por el fabricante.",
                "Reemplazar el SSD.",
            );
        } else if w >= 70 {
            push(
                out,
                Level::Warning,
                "Disco",
                format!("{name}: {w}% de vida útil consumida"),
                "El SSD lleva la mayor parte de su vida útil.",
                "Mantener respaldos al día y presupuestar el reemplazo.",
            );
        }
    }

    if let Some(t) = d.temperature_c {
        let (level, action) = match t {
            70.. => (
                Level::Critical,
                "Revisar ventilación y disipación del disco; a esta temperatura reduce velocidad y vida útil.",
            ),
            60..70 => (
                Level::Warning,
                "Limpiar ventiladores y revisar el flujo de aire del equipo.",
            ),
            _ => return,
        };
        push(
            out,
            level,
            "Disco",
            format!("{name}: temperatura de {t} °C"),
            "La temperatura del disco está por encima de lo recomendable.",
            action,
        );
    }
}

fn volume(v: &Volume, out: &mut Vec<Finding>) {
    let free_pct = 100.0 - v.used_pct();
    let label = if v.system {
        format!("{} (sistema)", v.mount)
    } else {
        v.mount.clone()
    };
    let detail = format!(
        "Quedan {} libres de {} ({free_pct:.0}%).",
        bytes(v.available_bytes),
        bytes(v.total_bytes)
    );
    let consequence = if v.system {
        " Con poco espacio el sistema se vuelve lento y las actualizaciones fallan."
    } else {
        ""
    };

    if free_pct < 5.0 || (v.system && v.available_bytes < 5 * GIB) {
        push(
            out,
            Level::Critical,
            "Almacenamiento",
            format!("{label}: casi sin espacio"),
            format!("{detail}{consequence}"),
            "Liberar espacio: archivos temporales, descargas, programas que no se usan.",
        );
    } else if free_pct < 15.0 {
        push(
            out,
            Level::Warning,
            "Almacenamiento",
            format!("{label}: poco espacio libre"),
            format!("{detail}{consequence}"),
            "Liberar espacio o mover archivos grandes a otra unidad.",
        );
    }

    if v.system && v.media == Media::Hdd {
        push(
            out,
            Level::Warning,
            "Almacenamiento",
            "El sistema está instalado en un disco duro mecánico",
            "Un disco mecánico es el principal motivo de lentitud al arrancar y abrir programas.",
            "Cambiar a un SSD: es la mejora de rendimiento más grande por el precio.",
        );
    }
}

fn memory(r: &Report, out: &mut Vec<Finding>) {
    let total = r.memory.total_bytes;
    // El firmware y la gráfica integrada reservan parte; 7.5 GiB visibles son 8 GiB físicos.
    if total > 0 && total < 3 * GIB + GIB / 2 {
        push(
            out,
            Level::Critical,
            "Memoria",
            format!("Solo {} de RAM", bytes(total)),
            "Es insuficiente para un sistema actual con navegador abierto.",
            "Ampliar a 8 GB como mínimo.",
        );
    } else if total > 0 && total < 7 * GIB + GIB / 2 {
        push(
            out,
            Level::Warning,
            "Memoria",
            format!("{} de RAM", bytes(total)),
            "Alcanza para uso básico, pero se queda corto con varias pestañas o programas a la vez.",
            "Ampliar a 8 o 16 GB si la placa lo permite.",
        );
    }

    let modules = r.memory.modules.len() as u32;
    if modules == 1 && r.memory.slots.is_some_and(|s| s >= 2) {
        push(
            out,
            Level::Info,
            "Memoria",
            "Un solo módulo de RAM",
            "Con un módulo la memoria trabaja en canal simple, con la mitad del ancho de banda.",
            "Al ampliar, agregar un segundo módulo igual para activar doble canal.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Cpu, Machine, Memory, MemoryModule, Os};

    fn report() -> Report {
        Report {
            generated_at: String::new(),
            tool_version: String::new(),
            machine: Machine::default(),
            os: Os::default(),
            cpu: Cpu::default(),
            memory: Memory {
                total_bytes: 16 * GIB,
                slots: Some(2),
                modules: vec![MemoryModule::default(), MemoryModule::default()],
            },
            disks: vec![],
            volumes: vec![],
            notes: vec![],
        }
    }

    fn disk() -> PhysicalDisk {
        PhysicalDisk {
            model: "SSD".into(),
            serial: None,
            size_bytes: 512 * GIB,
            media: Media::Ssd,
            bus: None,
            health: Health::Ok,
            temperature_c: Some(40),
            wear_pct: Some(5),
            power_on_hours: None,
            uncorrected_errors: Some(0),
        }
    }

    fn system_volume(total: u64, avail: u64, media: Media) -> Volume {
        Volume {
            mount: "C:\\".into(),
            file_system: "NTFS".into(),
            total_bytes: total,
            available_bytes: avail,
            media,
            removable: false,
            system: true,
        }
    }

    fn levels(r: &Report) -> Vec<Level> {
        analyze(r).into_iter().map(|f| f.level).collect()
    }

    #[test]
    fn healthy_machine_has_no_findings() {
        let mut r = report();
        r.disks.push(disk());
        r.volumes
            .push(system_volume(500 * GIB, 200 * GIB, Media::Ssd));
        assert!(analyze(&r).is_empty());
    }

    #[test]
    fn failing_disk_is_critical() {
        let mut r = report();
        r.disks.push(PhysicalDisk {
            health: Health::Failing,
            ..disk()
        });
        assert_eq!(levels(&r), [Level::Critical]);
    }

    #[test]
    fn wear_and_temperature_thresholds() {
        let mut r = report();
        r.disks.push(PhysicalDisk {
            wear_pct: Some(75),
            temperature_c: Some(72),
            ..disk()
        });
        assert_eq!(levels(&r), [Level::Critical, Level::Warning]);
    }

    #[test]
    fn system_volume_space() {
        let mut r = report();
        r.volumes
            .push(system_volume(100 * GIB, 4 * GIB, Media::Ssd));
        assert_eq!(levels(&r), [Level::Critical]);

        r.volumes[0].available_bytes = 12 * GIB;
        assert_eq!(levels(&r), [Level::Warning]);

        r.volumes[0].available_bytes = 40 * GIB;
        assert!(levels(&r).is_empty());
    }

    #[test]
    fn removable_volumes_are_ignored() {
        let mut r = report();
        r.volumes.push(Volume {
            removable: true,
            system: false,
            ..system_volume(16 * GIB, 0, Media::Unknown)
        });
        assert!(analyze(&r).is_empty());
    }

    #[test]
    fn small_partitions_are_ignored() {
        let mut r = report();
        r.volumes.push(Volume {
            mount: "/boot/efi".into(),
            system: false,
            ..system_volume(GIB / 2, GIB / 100, Media::Ssd)
        });
        assert!(analyze(&r).is_empty());
    }

    #[test]
    fn system_on_hdd_is_flagged() {
        let mut r = report();
        r.volumes
            .push(system_volume(1000 * GIB, 600 * GIB, Media::Hdd));
        let f = analyze(&r);
        assert_eq!(f.len(), 1);
        assert!(f[0].action.contains("SSD"));
    }

    #[test]
    fn memory_rules() {
        let mut r = report();
        r.memory.total_bytes = 3 * GIB + GIB / 4;
        assert_eq!(levels(&r), [Level::Critical]);

        // 8 GB físicos con gráfica integrada se ven como ~7.7 GiB: no se marca.
        r.memory.total_bytes = 7 * GIB + 7 * GIB / 10;
        assert!(levels(&r).is_empty());

        r.memory.modules.pop();
        assert_eq!(levels(&r), [Level::Info]);
    }

    #[test]
    fn findings_are_sorted_by_severity() {
        let mut r = report();
        r.memory.modules.pop();
        r.memory.total_bytes = 6 * GIB;
        r.disks.push(PhysicalDisk {
            health: Health::Failing,
            ..disk()
        });
        assert_eq!(levels(&r), [Level::Critical, Level::Warning, Level::Info]);
    }
}
