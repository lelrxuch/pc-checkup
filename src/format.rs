//! Formato de cantidades para humanos.

/// Bytes en unidades binarias con la etiqueta habitual (GB, no GiB), que es
/// como lo muestra Windows y como lo entiende un cliente.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Duración en días, horas y minutos, omitiendo los ceros a la izquierda.
pub fn duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{m} min"),
        (0, _) => format!("{h} h {m} min"),
        _ => format!("{d} d {h} h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(16 * 1024 * 1024 * 1024), "16.0 GB");
        assert_eq!(bytes(476 * 1024 * 1024 * 1024), "476 GB");
        assert_eq!(bytes(2 * 1024u64.pow(4)), "2.0 TB");
    }

    #[test]
    fn formats_duration() {
        assert_eq!(duration(59), "0 min");
        assert_eq!(duration(3 * 3600 + 5 * 60), "3 h 5 min");
        assert_eq!(duration(2 * 86_400 + 4 * 3600), "2 d 4 h");
    }
}
