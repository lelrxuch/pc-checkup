//! Detalle de hardware en Windows vía WMI. Cada consulta falla por separado:
//! un dato que no se pudo leer queda en `None` y se anota, el resto sigue.

use super::{PlatformData, codes};
use crate::report::{Machine, MemoryModule, PhysicalDisk};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use wmi::WMIConnection;

const STORAGE_NS: &str = "ROOT\\Microsoft\\Windows\\Storage";

#[derive(Deserialize)]
#[serde(rename = "Win32_ComputerSystem", rename_all = "PascalCase")]
struct ComputerSystem {
    manufacturer: Option<String>,
    model: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_BIOS", rename_all = "PascalCase")]
struct Bios {
    serial_number: Option<String>,
    #[serde(rename = "SMBIOSBIOSVersion")]
    smbios_bios_version: Option<String>,
    release_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_BaseBoard", rename_all = "PascalCase")]
struct BaseBoard {
    manufacturer: Option<String>,
    product: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_PhysicalMemory", rename_all = "PascalCase")]
struct PhysicalMemory {
    device_locator: Option<String>,
    capacity: Option<u64>,
    speed: Option<u32>,
    configured_clock_speed: Option<u32>,
    manufacturer: Option<String>,
    part_number: Option<String>,
    #[serde(rename = "SMBIOSMemoryType")]
    smbios_memory_type: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_PhysicalMemoryArray", rename_all = "PascalCase")]
struct PhysicalMemoryArray {
    memory_devices: Option<u16>,
    #[serde(rename = "Use")]
    usage: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename = "MSFT_PhysicalDisk", rename_all = "PascalCase")]
struct MsftPhysicalDisk {
    device_id: Option<String>,
    friendly_name: Option<String>,
    serial_number: Option<String>,
    size: Option<u64>,
    media_type: Option<u16>,
    bus_type: Option<u16>,
    health_status: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename = "MSFT_StorageReliabilityCounter", rename_all = "PascalCase")]
struct ReliabilityCounter {
    device_id: Option<String>,
    temperature: Option<u8>,
    wear: Option<u8>,
    power_on_hours: Option<u32>,
    read_errors_uncorrected: Option<u64>,
}

/// `Win32_PhysicalMemoryArray.Use` para la memoria del sistema.
const MEMORY_USE_SYSTEM: u16 = 3;

pub(crate) fn collect() -> PlatformData {
    let mut data = PlatformData::default();

    let cimv2 = match WMIConnection::new() {
        Ok(c) => c,
        Err(e) => {
            data.notes.push(format!(
                "No se pudo conectar a WMI ({e}); falta el detalle de hardware."
            ));
            return data;
        }
    };

    data.machine = machine(&cimv2, &mut data.notes);
    memory(&cimv2, &mut data);
    disks(&mut data);
    data
}

fn query<T: DeserializeOwned>(con: &WMIConnection, notes: &mut Vec<String>, what: &str) -> Vec<T> {
    con.query().unwrap_or_else(|e| {
        notes.push(format!("No se pudo leer {what}: {e}"));
        Vec::new()
    })
}

fn machine(con: &WMIConnection, notes: &mut Vec<String>) -> Machine {
    let cs = query::<ComputerSystem>(con, notes, "el modelo del equipo")
        .into_iter()
        .next();
    let bios = query::<Bios>(con, notes, "el BIOS").into_iter().next();
    let board = query::<BaseBoard>(con, notes, "la placa base")
        .into_iter()
        .next();

    let board = board.and_then(|b| {
        let parts: Vec<String> = [codes::clean(b.manufacturer), codes::clean(b.product)]
            .into_iter()
            .flatten()
            .collect();
        (!parts.is_empty()).then(|| parts.join(" "))
    });

    let (manufacturer, model) = cs
        .map(|c| (codes::clean(c.manufacturer), codes::clean(c.model)))
        .unwrap_or_default();
    let (serial, bios_version, bios_date) = bios
        .map(|b| {
            (
                codes::clean(b.serial_number),
                codes::clean(b.smbios_bios_version),
                b.release_date.as_deref().and_then(codes::cim_date),
            )
        })
        .unwrap_or_default();

    Machine {
        manufacturer,
        model,
        serial,
        board,
        bios_version,
        bios_date,
    }
}

fn memory(con: &WMIConnection, data: &mut PlatformData) {
    let arrays = query::<PhysicalMemoryArray>(con, &mut data.notes, "las ranuras de memoria");
    let slots: u32 = arrays
        .iter()
        .filter(|a| a.usage.is_none_or(|u| u == MEMORY_USE_SYSTEM))
        .filter_map(|a| a.memory_devices.map(u32::from))
        .sum();
    data.memory_slots = (slots > 0).then_some(slots);

    data.memory_modules = query::<PhysicalMemory>(con, &mut data.notes, "los módulos de memoria")
        .into_iter()
        .map(|m| MemoryModule {
            slot: codes::clean(m.device_locator),
            capacity_bytes: m.capacity.unwrap_or(0),
            // ConfiguredClockSpeed es la velocidad a la que corre; Speed, la nominal.
            speed_mhz: m.configured_clock_speed.filter(|&s| s > 0).or(m.speed),
            kind: m
                .smbios_memory_type
                .and_then(codes::memory_type_from_smbios)
                .map(str::to_string),
            manufacturer: codes::clean(m.manufacturer),
            part_number: codes::clean(m.part_number),
        })
        .collect();
}

fn disks(data: &mut PlatformData) {
    let storage = match WMIConnection::with_namespace_path(STORAGE_NS) {
        Ok(c) => c,
        Err(e) => {
            data.notes
                .push(format!("No se pudo leer la salud de los discos ({e})."));
            return;
        }
    };

    let physical = query::<MsftPhysicalDisk>(&storage, &mut data.notes, "los discos físicos");
    // Requiere permisos de administrador; sin ellos la consulta falla o viene vacía.
    let counters: Vec<ReliabilityCounter> = storage.query().unwrap_or_default();
    if counters.is_empty() && !physical.is_empty() {
        data.notes.push(
            "Ejecuta como administrador para ver temperatura, desgaste y horas de uso de los discos."
                .into(),
        );
    }

    data.disks = physical
        .into_iter()
        .map(|d| {
            let counter = counters
                .iter()
                .find(|c| c.device_id.is_some() && c.device_id == d.device_id);
            PhysicalDisk {
                model: codes::clean(d.friendly_name).unwrap_or_else(|| "Disco".into()),
                serial: codes::clean(d.serial_number),
                size_bytes: d.size.unwrap_or(0),
                media: codes::media_from_msft(d.media_type.unwrap_or(0)),
                bus: d
                    .bus_type
                    .and_then(codes::bus_from_msft)
                    .map(str::to_string),
                health: codes::health_from_msft(d.health_status.unwrap_or(u16::MAX)),
                temperature_c: counter
                    .and_then(|c| c.temperature)
                    .filter(|&t| t > 0)
                    .map(u32::from),
                wear_pct: counter.and_then(|c| c.wear).map(u32::from),
                power_on_hours: counter
                    .and_then(|c| c.power_on_hours)
                    .filter(|&h| h > 0)
                    .map(u64::from),
                uncorrected_errors: counter.and_then(|c| c.read_errors_uncorrected),
            }
        })
        .collect();
}
