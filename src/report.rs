//! Modelo de datos del reporte. Lo llena `collect`, lo evalúa `findings`
//! y lo dibuja `html`. Todo campo que el sistema puede no exponer es `Option`.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub generated_at: String,
    pub tool_version: String,
    pub machine: Machine,
    pub os: Os,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disks: Vec<PhysicalDisk>,
    pub volumes: Vec<Volume>,
    /// Limitaciones de la recolección (falta de permisos, herramientas ausentes).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Machine {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub board: Option<String>,
    pub bios_version: Option<String>,
    pub bios_date: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Os {
    pub name: Option<String>,
    pub version: Option<String>,
    pub kernel: Option<String>,
    pub hostname: Option<String>,
    pub arch: String,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Cpu {
    pub name: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    pub max_mhz: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Memory {
    pub total_bytes: u64,
    /// Ranuras físicas de la placa; `None` si el sistema no lo reporta.
    pub slots: Option<u32>,
    /// Vacío cuando no se pudo leer el detalle (en Linux requiere root).
    pub modules: Vec<MemoryModule>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MemoryModule {
    pub slot: Option<String>,
    pub capacity_bytes: u64,
    pub speed_mhz: Option<u32>,
    pub kind: Option<String>,
    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Media {
    Ssd,
    Hdd,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Ok,
    Warning,
    Failing,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhysicalDisk {
    pub model: String,
    pub serial: Option<String>,
    pub size_bytes: u64,
    pub media: Media,
    pub bus: Option<String>,
    pub health: Health,
    pub temperature_c: Option<u32>,
    /// Porcentaje de vida útil consumida (SSD).
    pub wear_pct: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub uncorrected_errors: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Volume {
    pub mount: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub media: Media,
    pub removable: bool,
    /// Volumen donde está instalado el sistema operativo.
    pub system: bool,
}

impl Volume {
    pub fn used_pct(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        100.0 * (self.total_bytes - self.available_bytes.min(self.total_bytes)) as f64
            / self.total_bytes as f64
    }
}
