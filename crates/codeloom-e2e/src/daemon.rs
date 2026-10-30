//! Limpieza de daemons huérfanos: el cliente de CodeLoom lanza `<exe> --server` suelto,
//! que sobrevive al test. Se identifica por la línea de órdenes y por `CODELOOM_HOME`.

use std::path::Path;
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const SETTLE: Duration = Duration::from_secs(3);

fn daemons_of(home: &Path) -> Vec<sysinfo::Pid> {
    let wanted = home.to_string_lossy().to_ascii_lowercase();
    // Cabe señalar que un perfil vacío casaría con cualquier proceso de `CODELOOM_HOME` vacío.
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_environ(UpdateKind::Always),
    );
    system
        .processes()
        .iter()
        .filter(|(_, process)| process.cmd().iter().any(|arg| arg == "--server"))
        .filter(|(_, process)| {
            process.environ().iter().any(|entry| {
                entry
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .strip_prefix("codeloom_home=")
                    .is_some_and(|value| value == wanted)
            })
        })
        .map(|(pid, _)| *pid)
        .collect()
}

/// Termina los procesos `--server` cuyo `CODELOOM_HOME` es `home`, espera a que
/// desaparezcan y devuelve cuántos mató.
pub fn kill_daemons_of(home: &Path) -> usize {
    let pids = daemons_of(home);
    if pids.is_empty() {
        return 0;
    }
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&pids), true);
    let mut killed = 0;
    for pid in &pids {
        if system.process(*pid).is_some_and(|process| process.kill()) {
            killed += 1;
        }
    }
    let started = Instant::now();
    while !daemons_of(home).is_empty() && started.elapsed() < SETTLE {
        std::thread::sleep(Duration::from_millis(50));
    }
    killed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kills_nothing_when_the_profile_has_no_daemon() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(kill_daemons_of(home.path()), 0);
    }

    #[cfg(windows)]
    #[test]
    fn an_empty_profile_path_kills_nothing() {
        // Un `--server` con `CODELOOM_HOME` vacío no debe casar con una ruta vacía.
        let mut orphan = std::process::Command::new("cmd.exe")
            .args(["/c", "pause", "--server"])
            .env("CODELOOM_HOME", "")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));

        assert_eq!(kill_daemons_of(std::path::Path::new("")), 0);
        assert!(
            orphan.try_wait().unwrap().is_none(),
            "se mató un proceso ajeno"
        );
        orphan.kill().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn kills_only_the_daemon_of_the_given_profile() {
        let mine = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        // Un proceso con `--server` en su línea de órdenes y el perfil de `mine`.
        let spawn = |home: &std::path::Path| {
            std::process::Command::new("cmd.exe")
                .args(["/c", "pause", "--server"])
                .env("CODELOOM_HOME", home)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        };
        let mut a = spawn(mine.path());
        let mut b = spawn(other.path());
        std::thread::sleep(std::time::Duration::from_millis(500));

        assert_eq!(kill_daemons_of(mine.path()), 1);
        assert!(a.wait().is_ok(), "el proceso de `mine` debía morir");
        assert!(b.try_wait().unwrap().is_none(), "el de `other` seguía vivo");
        b.kill().unwrap();
    }
}
