//! Obtención acotada del diff de la sesión: plazo, cota de bytes y ficheros máximos.

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::{parse_git_diff, DiffOutcome};

/// Número máximo de ficheros que se piden a `git diff`.
const MAX_FILES: usize = 20;
/// Plazo total de la llamada a `git`.
const TIMEOUT: Duration = Duration::from_secs(2);
/// Bytes máximos de salida que se conservan; el resto se descarta.
const MAX_BYTES: usize = 64 * 1024;
/// Bandera de Windows que evita abrir una ventana de consola para `git`.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Fallo de `run_bounded` que no es una salida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunError {
    /// El proceso no pudo lanzarse (no existe el ejecutable, etc.).
    Spawn,
    /// El proceso no terminó dentro del plazo y fue terminado.
    Timeout,
}

/// Pide a `git` el diff de `files` dentro de `cwd` y lo analiza.
///
/// Nunca bloquea más de `TIMEOUT`. Sin ficheros no lanza nada.
pub fn fetch_session_diff(cwd: &Path, files: &[String]) -> DiffOutcome {
    if files.is_empty() {
        return DiffOutcome::Ok(Vec::new());
    }
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(cwd).args(diff_args(files));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match run_bounded(cmd, TIMEOUT, MAX_BYTES) {
        // Un `git diff` correcto sale con 0. Un código distinto (128 si no hay
        // repositorio; 129 si `git diff` no reconoce el contexto sin repositorio)
        // indica que no hay diff utilizable.
        Err(RunError::Spawn) => DiffOutcome::NoGit,
        Ok((Some(codigo), _)) if codigo != 0 => DiffOutcome::NoGit,
        Err(RunError::Timeout) => DiffOutcome::TimedOut,
        Ok((_, salida)) => {
            let texto = String::from_utf8_lossy(&salida);
            DiffOutcome::Ok(parse_git_diff(&texto))
        }
    }
}

/// Construye los argumentos de `git diff`, limitados a `MAX_FILES` ficheros.
fn diff_args(files: &[String]) -> Vec<String> {
    let mut args: Vec<String> = ["diff", "--no-color", "-U2", "--"]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    args.extend(files.iter().take(MAX_FILES).cloned());
    args
}

/// Ejecuta `cmd` con plazo y cota de bytes, y devuelve el código de salida y la salida leída.
///
/// Lo que supere `max_bytes` se lee y se descarta para que el proceso no se
/// bloquee al llenar la tubería.
pub(crate) fn run_bounded(
    mut cmd: Command,
    timeout: Duration,
    max_bytes: usize,
) -> Result<(Option<i32>, Vec<u8>), RunError> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut hijo = cmd.spawn().map_err(|_| RunError::Spawn)?;
    let Some(mut salida) = hijo.stdout.take() else {
        terminar(&mut hijo);
        return Err(RunError::Spawn);
    };

    // La lectura va en un hilo aparte para que el plazo pueda vencer aunque el
    // proceso no cierre su salida. Tras el tope se sigue drenando la tubería.
    let limite_bytes = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = salida.by_ref().take(limite_bytes).read_to_end(&mut buf);
        let _ = io::copy(&mut salida, &mut io::sink());
        let _ = tx.send(buf);
    });

    let limite = Instant::now() + timeout;
    // Un fallo del hilo lector (pánico) se trata como vencimiento: no llega su salida.
    match rx.recv_timeout(timeout) {
        Ok(buf) => esperar_hasta(&mut hijo, limite).map(|codigo| (codigo, buf)),
        Err(_) => {
            terminar(&mut hijo);
            Err(RunError::Timeout)
        }
    }
}

/// Espera al proceso hasta `limite` sin bloquear más allá; si vence, lo termina.
fn esperar_hasta(hijo: &mut Child, limite: Instant) -> Result<Option<i32>, RunError> {
    loop {
        match hijo.try_wait() {
            Ok(Some(estado)) => return Ok(estado.code()),
            Ok(None) if Instant::now() < limite => thread::sleep(Duration::from_millis(5)),
            _ => {
                terminar(hijo);
                return Err(RunError::Timeout);
            }
        }
    }
}

/// Termina el proceso y recoge su estado; los errores se ignoran a propósito.
fn terminar(hijo: &mut Child) {
    let _ = hijo.kill();
    let _ = hijo.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    /// Indica si `git` está disponible en esta máquina.
    fn git_disponible() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|salida| salida.status.success())
            .unwrap_or(false)
    }

    /// Comando que no termina por sí solo, para probar el plazo.
    #[cfg(windows)]
    fn comando_lento() -> Command {
        let mut cmd = Command::new("ping");
        cmd.args(["-n", "30", "127.0.0.1"]);
        cmd
    }

    /// Comando que no termina por sí solo, para probar el plazo.
    #[cfg(not(windows))]
    fn comando_lento() -> Command {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        cmd
    }

    /// Ejecuta `git` con identidad y sin firma dentro de `dir`; devuelve si tuvo éxito.
    fn git(dir: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=Loom",
                "-c",
                "user.email=loom@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn files_vacio_no_lanza_nada() {
        // El directorio no existe: si se lanzara git, el resultado sería NoGit.
        let inexistente = PathBuf::from("G:/loomcoding-no-existe/ruta/inexistente");
        assert_eq!(
            fetch_session_diff(&inexistente, &[]),
            DiffOutcome::Ok(vec![])
        );
    }

    #[test]
    fn diff_args_pide_como_mucho_veinte_ficheros() {
        let files: Vec<String> = (0..25).map(|i| format!("f{i}.rs")).collect();
        let args = diff_args(&files);
        let inicio = args
            .iter()
            .position(|a| a == "--")
            .expect("falta el separador --");
        let pedidos = &args[inicio + 1..];
        assert_eq!(pedidos.len(), 20);
        assert_eq!(pedidos.first().map(String::as_str), Some("f0.rs"));
        assert_eq!(pedidos.last().map(String::as_str), Some("f19.rs"));
        assert_eq!(&args[..inicio], ["diff", "--no-color", "-U2"]);
    }

    #[test]
    fn run_bounded_devuelve_timeout_si_el_proceso_no_termina() {
        let inicio = Instant::now();
        let res = run_bounded(comando_lento(), Duration::from_millis(300), 1024);
        assert_eq!(res, Err(RunError::Timeout));
        assert!(inicio.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn run_bounded_descarta_lo_que_pase_del_limite() {
        if !git_disponible() {
            return;
        }
        let mut cmd = Command::new("git");
        cmd.arg("--version");
        let (codigo, salida) =
            run_bounded(cmd, Duration::from_secs(10), 3).expect("git debe ejecutarse");
        assert_eq!(codigo, Some(0));
        assert_eq!(salida, b"git".to_vec());
    }

    #[test]
    fn run_bounded_comando_inexistente_es_spawn() {
        let cmd = Command::new("comando-que-no-existe-loomcoding-7f3a");
        let res = run_bounded(cmd, Duration::from_secs(2), 1024);
        assert_eq!(res, Err(RunError::Spawn));
    }

    #[test]
    fn fetch_fuera_de_repo_es_no_git() {
        let dir = tempfile::tempdir().expect("directorio temporal");
        let files = vec!["a.txt".to_string()];
        assert_eq!(fetch_session_diff(dir.path(), &files), DiffOutcome::NoGit);
    }

    #[test]
    fn fetch_en_repo_real_devuelve_un_cambio() {
        if !git_disponible() {
            return;
        }
        let dir = tempfile::tempdir().expect("directorio temporal");
        let ruta = dir.path();
        assert!(git(ruta, &["init", "-q"]));
        std::fs::write(ruta.join("a.txt"), "uno\ndos\n").expect("escritura inicial");
        assert!(git(ruta, &["add", "a.txt"]));
        assert!(git(ruta, &["commit", "-q", "-m", "inicial"]));
        std::fs::write(ruta.join("a.txt"), "uno\ntres\n").expect("escritura modificada");

        let files = vec!["a.txt".to_string()];
        match fetch_session_diff(ruta, &files) {
            DiffOutcome::Ok(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0].path, "a.txt");
                assert_eq!(v[0].added, 1);
                assert_eq!(v[0].removed, 1);
            }
            otro => panic!("se esperaba Ok, llegó {otro:?}"),
        }
    }
}
