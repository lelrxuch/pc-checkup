//! Traducción de códigos y limpieza de cadenas del firmware. Vive fuera de
//! `windows.rs` para poder probarse en cualquier plataforma.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::report::{Health, Media};

/// Valores de relleno que los fabricantes dejan en el SMBIOS.
const PLACEHOLDERS: &[&str] = &[
    "to be filled by o.e.m.",
    "default string",
    "system product name",
    "system manufacturer",
    "system serial number",
    "base board serial number",
    "not specified",
    "not applicable",
    "none",
    "o.e.m.",
    "oem",
    "invalid",
    "unknown",
    "0",
    "123456789",
];

/// Recorta espacios y descarta valores vacíos o de relleno.
pub fn clean(value: Option<String>) -> Option<String> {
    let v = value?.trim().trim_matches(char::from(0)).trim().to_string();
    if v.is_empty() || PLACEHOLDERS.contains(&v.to_lowercase().as_str()) {
        None
    } else {
        Some(v)
    }
}

/// `MSFT_PhysicalDisk.MediaType`.
pub fn media_from_msft(code: u16) -> Media {
    match code {
        3 => Media::Hdd,
        4 | 5 => Media::Ssd,
        _ => Media::Unknown,
    }
}

/// `MSFT_PhysicalDisk.BusType`.
pub fn bus_from_msft(code: u16) -> Option<&'static str> {
    Some(match code {
        1 => "SCSI",
        3 => "ATA",
        7 => "USB",
        8 => "RAID",
        10 => "SAS",
        11 => "SATA",
        12 => "SD",
        13 => "MMC",
        17 => "NVMe",
        _ => return None,
    })
}

/// `MSFT_PhysicalDisk.HealthStatus`.
pub fn health_from_msft(code: u16) -> Health {
    match code {
        0 => Health::Ok,
        1 => Health::Warning,
        2 => Health::Failing,
        _ => Health::Unknown,
    }
}

/// `Win32_PhysicalMemory.SMBIOSMemoryType` (tabla 7.18.2 de SMBIOS).
pub fn memory_type_from_smbios(code: u32) -> Option<&'static str> {
    Some(match code {
        18 => "DDR",
        19 => "DDR2",
        24 => "DDR3",
        26 => "DDR4",
        27 => "LPDDR",
        28 => "LPDDR2",
        29 => "LPDDR3",
        30 => "LPDDR4",
        34 => "DDR5",
        35 => "LPDDR5",
        _ => return None,
    })
}

/// Fecha CIM (`20230315000000.000000+000`) a `2023-03-15`.
pub fn cim_date(value: &str) -> Option<String> {
    let d = value.get(..8)?;
    if !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_drops_firmware_placeholders() {
        assert_eq!(clean(Some("  To Be Filled By O.E.M. ".into())), None);
        assert_eq!(clean(Some("Default string".into())), None);
        assert_eq!(clean(Some("".into())), None);
        assert_eq!(clean(None), None);
        assert_eq!(clean(Some(" HP \0".into())), Some("HP".into()));
    }

    #[test]
    fn msft_codes() {
        assert_eq!(media_from_msft(4), Media::Ssd);
        assert_eq!(media_from_msft(3), Media::Hdd);
        assert_eq!(media_from_msft(0), Media::Unknown);
        assert_eq!(bus_from_msft(17), Some("NVMe"));
        assert_eq!(bus_from_msft(99), None);
        assert_eq!(health_from_msft(2), Health::Failing);
        assert_eq!(health_from_msft(5), Health::Unknown);
    }

    #[test]
    fn smbios_memory_type() {
        assert_eq!(memory_type_from_smbios(26), Some("DDR4"));
        assert_eq!(memory_type_from_smbios(0), None);
    }

    #[test]
    fn cim_dates() {
        assert_eq!(
            cim_date("20230315000000.000000+000").as_deref(),
            Some("2023-03-15")
        );
        assert_eq!(cim_date("2023"), None);
        assert_eq!(cim_date("abcdefgh"), None);
    }
}
