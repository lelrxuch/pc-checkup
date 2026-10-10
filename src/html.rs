//! Reporte HTML autocontenido: un solo archivo, sin recursos externos, que
//! se puede abrir sin internet, enviar por correo o imprimir.

use crate::findings::{Finding, Level};
use crate::format::{bytes, duration};
use crate::report::{Health, Media, Report};
use std::fmt::Write;

const STYLE: &str = include_str!("report.css");

pub fn render(r: &Report, findings: &[Finding]) -> String {
    let mut h = String::with_capacity(16 * 1024);
    let title =
        r.os.hostname
            .clone()
            .unwrap_or_else(|| "Equipo".to_string());

    let _ = write!(
        h,
        "<!doctype html>\n<html lang=\"es\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Diagnóstico · {}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n",
        esc(&title)
    );

    header(&mut h, r, &title);
    summary(&mut h, findings);
    findings_section(&mut h, findings);
    machine(&mut h, r);
    memory(&mut h, r);
    disks(&mut h, r);
    volumes(&mut h, r);
    notes(&mut h, r);

    let _ = write!(
        h,
        "<footer>Generado con pc-checkup {} · {}</footer>\n</main>\n</body>\n</html>\n",
        esc(&r.tool_version),
        esc(&r.generated_at)
    );
    h
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn opt(v: &Option<String>) -> String {
    v.as_deref().map(esc).unwrap_or_else(|| "—".into())
}

fn header(h: &mut String, r: &Report, title: &str) {
    let model = [&r.machine.manufacturer, &r.machine.model]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let _ = write!(
        h,
        "<header>\n<p class=\"eyebrow\">Reporte de diagnóstico</p>\n<h1>{}</h1>\n",
        esc(title)
    );
    if !model.is_empty() {
        let _ = writeln!(h, "<p class=\"sub\">{}</p>", esc(&model));
    }
    let _ = writeln!(
        h,
        "<p class=\"meta\">{}</p>\n</header>",
        esc(&r.generated_at)
    );
}

fn level_class(l: Level) -> &'static str {
    match l {
        Level::Critical => "critical",
        Level::Warning => "warning",
        Level::Info => "info",
    }
}

fn level_label(l: Level) -> &'static str {
    match l {
        Level::Critical => "Crítico",
        Level::Warning => "Atención",
        Level::Info => "Sugerencia",
    }
}

fn summary(h: &mut String, findings: &[Finding]) {
    let count = |l| findings.iter().filter(|f| f.level == l).count();
    let (c, w, i) = (
        count(Level::Critical),
        count(Level::Warning),
        count(Level::Info),
    );
    let (class, text) = match (c, w) {
        (0, 0) => ("ok", "Sin problemas detectados".to_string()),
        (0, _) => (
            "warning",
            plural(w, "punto requiere atención", "puntos requieren atención"),
        ),
        _ => (
            "critical",
            plural(c, "problema crítico", "problemas críticos"),
        ),
    };
    let _ = write!(
        h,
        "<section class=\"summary {class}\">\n<p class=\"verdict\">{}</p>\n<ul class=\"counts\">\
         <li><b>{c}</b> críticos</li><li><b>{w}</b> de atención</li><li><b>{i}</b> sugerencias</li></ul>\n</section>\n",
        esc(&text)
    );
}

/// 14820 → "14 820" (espacio fino como separador, legible en cualquier idioma).
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(c);
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

fn findings_section(h: &mut String, findings: &[Finding]) {
    if findings.is_empty() {
        return;
    }
    h.push_str("<section>\n<h2>Hallazgos</h2>\n<ol class=\"findings\">\n");
    for f in findings {
        let _ = write!(
            h,
            "<li class=\"finding {cls}\">\n<p class=\"tag\">{lvl} · {area}</p>\n<h3>{title}</h3>\n\
             <p>{detail}</p>\n<p class=\"action\"><b>Qué hacer:</b> {action}</p>\n</li>\n",
            cls = level_class(f.level),
            lvl = level_label(f.level),
            area = esc(f.area),
            title = esc(&f.title),
            detail = esc(&f.detail),
            action = esc(&f.action),
        );
    }
    h.push_str("</ol>\n</section>\n");
}

fn kv(h: &mut String, rows: &[(&str, String)]) {
    h.push_str("<dl class=\"kv\">\n");
    for (k, v) in rows {
        let _ = writeln!(h, "<dt>{}</dt><dd>{v}</dd>", esc(k));
    }
    h.push_str("</dl>\n");
}

fn machine(h: &mut String, r: &Report) {
    let m = &r.machine;
    let bios = match (&m.bios_version, &m.bios_date) {
        (Some(v), Some(d)) => format!("{} ({})", esc(v), esc(d)),
        (v, d) => opt(&v.clone().or(d.clone())),
    };
    let cores = match r.cpu.physical_cores {
        Some(p) => format!("{p} núcleos, {} hilos", r.cpu.logical_cores),
        None => format!("{} hilos", r.cpu.logical_cores),
    };
    let cpu = match r.cpu.max_mhz {
        Some(mhz) => format!(
            "{} · {cores} · {:.1} GHz",
            esc(&r.cpu.name),
            mhz as f64 / 1000.0
        ),
        None => format!("{} · {cores}", esc(&r.cpu.name)),
    };
    let os = [&r.os.name, &r.os.version]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    // `long_os_version` ya suele incluir la versión; no repetirla.
    let os = match os.as_slice() {
        [name, ver] if name.contains(ver.as_str()) => name.clone(),
        parts => parts.join(" "),
    };

    h.push_str("<section>\n<h2>Equipo</h2>\n");
    let mut rows = vec![
        ("Fabricante", opt(&m.manufacturer)),
        ("Modelo", opt(&m.model)),
        ("Número de serie", opt(&m.serial)),
        ("Placa base", opt(&m.board)),
        ("BIOS", bios),
        ("Procesador", cpu),
        (
            "Sistema operativo",
            if os.is_empty() {
                "—".into()
            } else {
                esc(&os)
            },
        ),
    ];
    // Windows no expone un kernel aparte del número de versión.
    if r.os.kernel.is_some() && !cfg!(windows) {
        rows.push(("Kernel", opt(&r.os.kernel)));
    }
    rows.push(("Arquitectura", esc(&r.os.arch)));
    rows.push(("Encendido desde hace", esc(&duration(r.os.uptime_secs))));
    kv(h, &rows);
    h.push_str("</section>\n");
}

fn memory(h: &mut String, r: &Report) {
    let mem = &r.memory;
    h.push_str("<section>\n<h2>Memoria</h2>\n");
    let slots = match mem.slots {
        Some(s) if !mem.modules.is_empty() => format!("{} de {s} ocupadas", mem.modules.len()),
        Some(s) => format!("{s}"),
        None => "—".into(),
    };
    kv(
        h,
        &[("Total", esc(&bytes(mem.total_bytes))), ("Ranuras", slots)],
    );
    if !mem.modules.is_empty() {
        h.push_str(
            "<table>\n<thead><tr><th>Ranura</th><th>Capacidad</th><th>Tipo</th>\
             <th>Velocidad</th><th>Fabricante</th><th>Parte</th></tr></thead>\n<tbody>\n",
        );
        for m in &mem.modules {
            let _ = writeln!(
                h,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                opt(&m.slot),
                esc(&bytes(m.capacity_bytes)),
                opt(&m.kind),
                m.speed_mhz
                    .map(|s| format!("{s} MT/s"))
                    .unwrap_or_else(|| "—".into()),
                opt(&m.manufacturer),
                opt(&m.part_number),
            );
        }
        h.push_str("</tbody>\n</table>\n");
    }
    h.push_str("</section>\n");
}

fn media_label(m: Media) -> &'static str {
    match m {
        Media::Ssd => "SSD",
        Media::Hdd => "HDD",
        Media::Unknown => "—",
    }
}

fn health_badge(hl: Health) -> &'static str {
    match hl {
        Health::Ok => "<span class=\"badge ok\">Buena</span>",
        Health::Warning => "<span class=\"badge warning\">Degradada</span>",
        Health::Failing => "<span class=\"badge critical\">Falla</span>",
        Health::Unknown => "<span class=\"badge unknown\">Sin datos</span>",
    }
}

fn disks(h: &mut String, r: &Report) {
    if r.disks.is_empty() {
        return;
    }
    h.push_str(
        "<section>\n<h2>Discos</h2>\n<table>\n<thead><tr><th>Modelo</th><th>Tipo</th>\
         <th>Capacidad</th><th>Salud</th><th>Temp.</th><th>Desgaste</th><th>Horas de uso</th>\
         </tr></thead>\n<tbody>\n",
    );
    for d in &r.disks {
        let kind = match &d.bus {
            Some(b) if d.media != Media::Unknown => format!("{} {}", media_label(d.media), esc(b)),
            Some(b) => esc(b),
            None => media_label(d.media).into(),
        };
        let dash = || "—".to_string();
        let model = match &d.serial {
            Some(s) => format!("{}<small>{}</small>", esc(&d.model), esc(s)),
            None => esc(&d.model),
        };
        let _ = writeln!(
            h,
            "<tr><td>{model}</td><td>{kind}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&bytes(d.size_bytes)),
            health_badge(d.health),
            d.temperature_c
                .map(|t| format!("{t} °C"))
                .unwrap_or_else(dash),
            d.wear_pct.map(|w| format!("{w}%")).unwrap_or_else(dash),
            d.power_on_hours.map(thousands).unwrap_or_else(dash),
        );
    }
    h.push_str("</tbody>\n</table>\n</section>\n");
}

fn volumes(h: &mut String, r: &Report) {
    if r.volumes.is_empty() {
        return;
    }
    h.push_str("<section>\n<h2>Unidades</h2>\n<ul class=\"volumes\">\n");
    for v in &r.volumes {
        let used = v.used_pct();
        let cls = match 100.0 - used {
            f if f < 5.0 => " critical",
            f if f < 15.0 => " warning",
            _ => "",
        };
        let mut tags = vec![esc(&v.file_system)];
        if v.media != Media::Unknown {
            tags.push(media_label(v.media).into());
        }
        if v.system {
            tags.push("Sistema".into());
        }
        if v.removable {
            tags.push("Extraíble".into());
        }
        let _ = write!(
            h,
            "<li>\n<div class=\"vol-head\"><b>{}</b><span>{}</span></div>\n\
             <div class=\"bar{cls}\"><span style=\"width:{used:.1}%\"></span></div>\n\
             <p class=\"vol-foot\">{} libres de {}</p>\n</li>\n",
            esc(&v.mount),
            tags.join(" · "),
            esc(&bytes(v.available_bytes)),
            esc(&bytes(v.total_bytes)),
        );
    }
    h.push_str("</ul>\n</section>\n");
}

fn notes(h: &mut String, r: &Report) {
    if r.notes.is_empty() {
        return;
    }
    h.push_str("<section class=\"notes\">\n<h2>Limitaciones de este reporte</h2>\n<ul>\n");
    for n in &r.notes {
        let _ = writeln!(h, "<li>{}</li>", esc(n));
    }
    h.push_str("</ul>\n</section>\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::findings::analyze;
    use crate::report::*;

    fn sample() -> Report {
        Report {
            generated_at: "2026-10-10 15:30".into(),
            tool_version: "0.1.0".into(),
            machine: Machine {
                manufacturer: Some("HP".into()),
                model: Some("<script>alert(1)</script>".into()),
                ..Default::default()
            },
            os: Os {
                hostname: Some("PC-Cliente".into()),
                arch: "x86_64".into(),
                ..Default::default()
            },
            cpu: Cpu {
                name: "Intel Core i5".into(),
                logical_cores: 8,
                ..Default::default()
            },
            memory: Memory {
                total_bytes: 8 << 30,
                ..Default::default()
            },
            disks: vec![PhysicalDisk {
                model: "WDC".into(),
                serial: None,
                size_bytes: 1 << 40,
                media: Media::Hdd,
                bus: Some("SATA".into()),
                health: Health::Failing,
                temperature_c: None,
                wear_pct: None,
                power_on_hours: Some(30000),
                uncorrected_errors: None,
            }],
            volumes: vec![],
            notes: vec!["Sin permisos".into()],
        }
    }

    #[test]
    fn escapes_untrusted_values() {
        let html = render(&sample(), &[]);
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn includes_findings_and_sections() {
        let r = sample();
        let html = render(&r, &analyze(&r));
        assert!(html.contains("1 problema crítico"));
        assert!(html.contains("Qué hacer:"));
        assert!(html.contains("HDD SATA"));
        assert!(html.contains("Limitaciones de este reporte"));
        assert!(html.contains("<title>Diagnóstico · PC-Cliente</title>"));
    }

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(610), "610");
        assert_eq!(thousands(14_820), "14\u{2009}820");
        assert_eq!(thousands(1_234_567), "1\u{2009}234\u{2009}567");
    }

    #[test]
    fn clean_report_says_so() {
        let mut r = sample();
        r.disks.clear();
        let html = render(&r, &analyze(&r));
        assert!(html.contains("Sin problemas detectados"));
        assert!(!html.contains("<h2>Hallazgos</h2>"));
    }
}
