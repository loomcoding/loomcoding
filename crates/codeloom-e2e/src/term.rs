//! Terminal de pruebas: un PTY con un emulador (`vt100`) que interpreta lo que pinta el hijo.

use crate::snapshot::Snapshot;
use anyhow::Context;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Margen que se da a la salida pendiente de procesar cuando el hijo ya ha terminado.
const EXIT_GRACE: Duration = Duration::from_millis(300);
const POLL: Duration = Duration::from_millis(25);

pub struct TermConfig {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

/// Teclas con secuencia fija en una terminal xterm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Enter,
    Esc,
    Tab,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    CtrlC,
    CtrlD,
    CtrlJ,
    F1,
}

impl Key {
    pub fn bytes(&self) -> &'static [u8] {
        match self {
            Key::Enter => b"\r",
            Key::Esc => b"\x1b",
            Key::Tab => b"\t",
            Key::Backspace => b"\x7f",
            Key::Up => b"\x1b[A",
            Key::Down => b"\x1b[B",
            Key::Right => b"\x1b[C",
            Key::Left => b"\x1b[D",
            Key::CtrlC => b"\x03",
            Key::CtrlD => b"\x04",
            Key::CtrlJ => b"\n",
            Key::F1 => b"\x1bOP",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("no apareció {needle:?} a tiempo; pantalla:\n{screen}")]
    Timeout { needle: String, screen: String },
    #[error(
        "el hijo terminó (código {code:?}) sin mostrar el texto esperado; pantalla:\n{screen}"
    )]
    ChildExited { code: Option<u32>, screen: String },
}

/// Si `chunk` contiene la consulta de posición del cursor (`ESC[6n`), la respuesta que
/// espera ConPTY: `ESC[fila;columnaR` con coordenadas desde 1. `cursor` es (fila, columna)
/// desde 0.
pub fn cpr_reply(chunk: &[u8], cursor: (u16, u16)) -> Option<Vec<u8>> {
    const QUERY: &[u8] = b"\x1b[6n";
    chunk
        .windows(QUERY.len())
        .any(|w| w == QUERY)
        .then(|| format!("\x1b[{};{}R", cursor.0 + 1, cursor.1 + 1).into_bytes())
}

/// Texto pegado con *bracketed paste*.
pub fn paste_bytes(text: &str) -> Vec<u8> {
    let mut out = b"\x1b[200~".to_vec();
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}

pub struct Term {
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    exited_at: Mutex<Option<Instant>>,
}

impl Term {
    pub fn spawn(cfg: TermConfig) -> anyhow::Result<Term> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: cfg.rows,
                cols: cfg.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("no se pudo abrir el PTY")?;
        let mut command = CommandBuilder::new(&cfg.program);
        command.args(&cfg.args);
        command.cwd(&cfg.cwd);
        for (key, value) in &cfg.env {
            command.env(key, value);
        }
        let child = pair
            .slave
            .spawn_command(command)
            .with_context(|| format!("no se pudo lanzar {}", cfg.program.display()))?;
        // Soltar el lado esclavo para que el lector vea el cierre cuando el hijo muera.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let parser = Arc::new(Mutex::new(vt100::Parser::new(cfg.rows, cfg.cols, 0)));

        let (parser_r, writer_r) = (parser.clone(), writer.clone());
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let chunk = &buf[..n];
                let reply = {
                    let mut parser = parser_r.lock().unwrap();
                    parser.process(chunk);
                    cpr_reply(chunk, parser.screen().cursor_position())
                };
                if let Some(reply) = reply {
                    if let Ok(mut writer) = writer_r.lock() {
                        let _ = writer.write_all(&reply);
                        let _ = writer.flush();
                    }
                }
            }
        });

        Ok(Term {
            parser,
            writer,
            master: pair.master,
            child: Mutex::new(child),
            exited_at: Mutex::new(None),
        })
    }

    pub fn send_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    }

    pub fn send_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.send_bytes(text.as_bytes())
    }

    pub fn press(&mut self, key: Key) -> anyhow::Result<()> {
        self.send_bytes(key.bytes())
    }

    pub fn paste(&mut self, text: &str) -> anyhow::Result<()> {
        self.send_bytes(&paste_bytes(text))
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        self.parser.lock().unwrap().set_size(rows, cols);
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot::from_screen(self.parser.lock().unwrap().screen())
    }

    /// Código de salida del hijo si ya terminó.
    pub fn exit_code(&self) -> Option<u32> {
        let status = self.child.lock().unwrap().try_wait().ok().flatten()?;
        self.exited_at
            .lock()
            .unwrap()
            .get_or_insert_with(Instant::now);
        Some(status.exit_code())
    }

    pub fn wait_for_text(&self, needle: &str, timeout: Duration) -> Result<Snapshot, WaitError> {
        let started = Instant::now();
        loop {
            let snap = self.snapshot();
            if snap.text.contains(needle) {
                return Ok(snap);
            }
            if let Some(code) = self.exit_code() {
                let gone = self.exited_at.lock().unwrap().map(|t| t.elapsed());
                if gone.is_some_and(|elapsed| elapsed >= EXIT_GRACE) {
                    return Err(WaitError::ChildExited {
                        code: Some(code),
                        screen: snap.text,
                    });
                }
            }
            if started.elapsed() >= timeout {
                return Err(WaitError::Timeout {
                    needle: needle.to_string(),
                    screen: snap.text,
                });
            }
            std::thread::sleep(POLL);
        }
    }

    /// Escribe `<name>.txt` y `<name>.json` con la pantalla actual.
    pub fn save(&self, dir: &Path, name: &str) -> anyhow::Result<()> {
        std::fs::create_dir_all(dir)?;
        let snap = self.snapshot();
        std::fs::write(dir.join(format!("{name}.txt")), &snap.text)?;
        std::fs::write(dir.join(format!("{name}.json")), snap.to_json())?;
        Ok(())
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::time::{Duration, Instant};

    #[cfg(windows)]
    fn cmd(args: &[&str]) -> TermConfig {
        TermConfig {
            program: std::env::var_os("COMSPEC")
                .unwrap_or_else(|| "cmd.exe".into())
                .into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: std::env::temp_dir(),
            env: vec![],
            rows: 24,
            cols: 80,
        }
    }

    #[test]
    fn cpr_reply_answers_cursor_query() {
        assert_eq!(cpr_reply(b"x\x1b[6ny", (0, 0)), Some(b"\x1b[1;1R".to_vec()));
        assert_eq!(
            cpr_reply(b"x\x1b[6ny", (4, 9)),
            Some(b"\x1b[5;10R".to_vec())
        );
        assert_eq!(cpr_reply(b"nada", (0, 0)), None);
    }

    #[test]
    fn key_bytes() {
        assert_eq!(Key::Enter.bytes(), b"\r");
        assert_eq!(Key::Esc.bytes(), b"\x1b");
        assert_eq!(Key::CtrlC.bytes(), b"\x03");
        assert_eq!(Key::Up.bytes(), b"\x1b[A");
    }

    #[test]
    fn paste_wraps_in_bracketed_paste() {
        assert_eq!(paste_bytes("a\nb"), b"\x1b[200~a\nb\x1b[201~".to_vec());
    }

    #[cfg(windows)]
    #[test]
    fn spawn_echo_and_wait() {
        let term = Term::spawn(cmd(&["/c", "echo hola"])).unwrap();
        let snap = term.wait_for_text("hola", Duration::from_secs(10)).unwrap();
        assert!(snap.text.contains("hola"));
    }

    #[cfg(windows)]
    #[test]
    fn child_exit_before_text_fails_fast() {
        let term = Term::spawn(cmd(&["/c", "exit 3"])).unwrap();
        let started = Instant::now();
        let err = term
            .wait_for_text("nunca", Duration::from_secs(10))
            .unwrap_err();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "tardó {:?}",
            started.elapsed()
        );
        assert!(
            matches!(err, WaitError::ChildExited { code: Some(3), .. }),
            "{err:?}"
        );
    }
}
