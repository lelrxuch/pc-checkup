mod collect;
mod demo;
mod findings;
mod format;
mod html;
mod report;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Uso: pc-checkup [opciones]

Revisa el equipo y genera un reporte HTML en la carpeta actual.

Opciones:
  -o, --out RUTA   Archivo de salida (por defecto reporte-<equipo>-<fecha>.html)
      --json       Escribe los datos en JSON a la salida estándar, sin reporte HTML
      --no-open    No abre el reporte al terminar
      --demo       Usa datos de ejemplo en lugar de revisar este equipo
  -h, --help       Muestra esta ayuda
  -V, --version    Muestra la versión";

#[derive(Debug, Default, PartialEq)]
struct Args {
    out: Option<PathBuf>,
    json: bool,
    open: bool,
    demo: bool,
}

enum Cmd {
    Run(Args),
    Help,
    Version,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Cmd, String> {
    let mut a = Args {
        open: true,
        ..Default::default()
    };
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Cmd::Help),
            "-V" | "--version" => return Ok(Cmd::Version),
            "--json" => a.json = true,
            "--no-open" => a.open = false,
            "--demo" => a.demo = true,
            "-o" | "--out" => {
                let path = it.next().ok_or("falta la ruta después de --out")?;
                a.out = Some(path.into());
            }
            other => return Err(format!("opción desconocida: {other}")),
        }
    }
    Ok(Cmd::Run(a))
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(Cmd::Run(a)) => a,
        Ok(Cmd::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Cmd::Version) => {
            println!("pc-checkup {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("pc-checkup: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    let report = if args.demo {
        demo::report()
    } else {
        collect::collect()
    };
    let findings = findings::analyze(&report);

    if args.json {
        let doc = serde_json::json!({ "report": report, "findings": findings });
        println!(
            "{}",
            serde_json::to_string_pretty(&doc).expect("datos serializables")
        );
        return ExitCode::SUCCESS;
    }

    let path = args
        .out
        .unwrap_or_else(|| default_file_name(report.os.hostname.as_deref()).into());
    if let Err(e) = std::fs::write(&path, html::render(&report, &findings)) {
        eprintln!("pc-checkup: no se pudo escribir {}: {e}", path.display());
        return ExitCode::FAILURE;
    }
    restore_owner(&path);

    let shown = std::fs::canonicalize(&path).unwrap_or(path);
    println!("Reporte guardado en {}", shown.display());
    for f in &findings {
        println!("  [{:?}] {}", f.level, f.title);
    }
    if args.open {
        open(&shown);
    }
    ExitCode::SUCCESS
}

/// Con `sudo` el reporte quedaría a nombre de root y el usuario no podría
/// borrarlo ni editarlo; se devuelve a quien ejecutó sudo.
#[cfg(unix)]
fn restore_owner(path: &std::path::Path) {
    let id = |var| std::env::var(var).ok().and_then(|v| v.parse::<u32>().ok());
    if let (Some(uid), Some(gid)) = (id("SUDO_UID"), id("SUDO_GID")) {
        if let Err(e) = std::os::unix::fs::chown(path, Some(uid), Some(gid)) {
            eprintln!("No se pudo cambiar el dueño del reporte: {e}");
        }
    }
}

#[cfg(not(unix))]
fn restore_owner(_path: &std::path::Path) {}

fn default_file_name(hostname: Option<&str>) -> String {
    let host: String = hostname
        .unwrap_or("equipo")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M");
    format!("reporte-{host}-{stamp}.html")
}

fn open(path: &std::path::Path) {
    #[cfg(windows)]
    let result = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(path).spawn();

    if let Err(e) = result {
        eprintln!("No se pudo abrir el reporte automáticamente: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Args, String> {
        match parse_args(args.iter().map(|s| s.to_string()))? {
            Cmd::Run(a) => Ok(a),
            _ => Err("no es Run".into()),
        }
    }

    #[test]
    fn parses_options() {
        let a = run(&["--no-open", "-o", "x.html"]).unwrap();
        assert_eq!(
            a,
            Args {
                out: Some("x.html".into()),
                json: false,
                open: false,
                demo: false,
            }
        );
        assert!(run(&["--json"]).unwrap().json);
        assert!(run(&["--out"]).is_err());
        assert!(run(&["--nope"]).is_err());
    }

    #[test]
    fn file_name_is_safe() {
        let n = default_file_name(Some("PC de Ana/1"));
        assert!(n.starts_with("reporte-PC_de_Ana_1-"));
        assert!(n.ends_with(".html"));
    }
}
