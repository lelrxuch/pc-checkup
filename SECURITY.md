# Seguridad

pc-checkup se ejecuta, a veces como administrador, en equipos de terceros, así que su alcance es mínimo a propósito.

## Qué hace

- Lee información del sistema: WMI en Windows; `/sys` y `smartctl` en Linux.
- Escribe un único archivo HTML en la carpeta actual (o en `--out`) y lo abre en el navegador.
- No modifica configuraciones, no instala nada, no crea servicios ni tareas programadas.

## Qué no hace

- **No usa la red.** No hay telemetría, actualizaciones ni envío de reportes.
- No guarda nada fuera del reporte.

Para comprobarlo, el código no usa ninguna librería de red (`Cargo.toml`). En Windows también lo puedes ver con el Monitor de recursos, en la pestaña Red, mientras corre.

## Datos del reporte

El reporte incluye el número de serie del equipo y de los discos. Revísalo antes de compartirlo fuera de la relación con el cliente.

## Verificar una descarga

El `.exe` no está firmado, así que SmartScreen o el antivirus pueden avisar. Compara el hash con `SHA256SUMS.txt` de la misma release:

```powershell
Get-FileHash .\pc-checkup-*-windows-x64.exe
```

## Cadena de suministro

- Las acciones de GitHub están fijadas por hash de commit.
- Dependabot avisa de crates con versiones nuevas o vulnerables.

## Reportar una vulnerabilidad

Usa **Security → Report a vulnerability** en este repositorio.
