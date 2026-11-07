//! Registro de `tracing` a fichero (spec `docs/specs/2026-11-07-loomcoding-registro-tracing.md`).
//!
//! Solo se instala un suscriptor si `RUST_LOG` está definida y no vacía; sin ella no se crea
//! fichero ni directorio y el comportamiento es idéntico al de siempre. El destino es un fichero por
//! proceso (`<datos>/logs/loomcoding-<rol>.log`), acotado a `MAX_LOG_BYTES` con una sola copia
//! anterior (`.1`). Nunca se escribe en stdout ni en stderr: la TUI es dueña de la terminal.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

/// Cota del fichero de registro: al superarla se rota a `<nombre>.1`.
pub const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Testigo de la instalación. El suscriptor global vive hasta que termina el proceso.
#[derive(Debug)]
#[must_use = "el registro solo existe mientras se conserve el testigo"]
pub struct Guard;

/// Directorio de datos de Loomcoding (`<perfil>/.loomcoding`), con el mismo resolvedor que el resto.
pub fn dir_datos() -> PathBuf {
    crate::home::user_home()
        .unwrap_or_else(std::env::temp_dir)
        .join(".loomcoding")
}

/// Instala el suscriptor de fichero si `RUST_LOG` está definida y no vacía. Cualquier fallo (permisos,
/// disco, suscriptor ya instalado) se ignora en silencio: un registro roto nunca impide arrancar.
pub fn instala(rol: &str, dir_datos: &Path) -> Option<Guard> {
    instala_con(rol, dir_datos, std::env::var("RUST_LOG").ok().as_deref())
}

/// Igual que `instala`, con el valor de `RUST_LOG` recibido como argumento para poder probarla sin
/// tocar el entorno del proceso.
pub fn instala_con(rol: &str, dir_datos: &Path, rust_log: Option<&str>) -> Option<Guard> {
    let filtro = filtro_desde(rust_log)?;
    let ruta = dir_datos.join("logs").join(format!("loomcoding-{rol}.log"));
    let escritor = Escritor::abre(&ruta, MAX_LOG_BYTES).ok()?;
    tracing::subscriber::set_global_default(suscriptor(escritor, filtro)).ok()?;
    Some(Guard)
}

/// Filtro a partir de `RUST_LOG`. Ausente o vacía, no hay filtro y no se instala nada.
pub fn filtro_desde(valor: Option<&str>) -> Option<EnvFilter> {
    let valor = valor?.trim();
    if valor.is_empty() {
        return None;
    }
    Some(EnvFilter::builder().parse_lossy(valor))
}

/// Suscriptor que escribe solo al fichero (sin color, sin stdout ni stderr).
pub fn suscriptor(escritor: Escritor, filtro: EnvFilter) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::registry().with(filtro).with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(escritor),
    )
}

/// Estado compartido del fichero: ruta, copia anterior, cota y el tamaño que lleva escrito.
struct Estado {
    actual: PathBuf,
    viejo: PathBuf,
    limite: u64,
    fichero: Option<File>,
    tamano: u64,
}

impl Estado {
    /// Escribe un evento completo. Si no cabe en el fichero actual, rota antes; cada evento se
    /// recorta a la cota, así que el fichero nunca la supera.
    fn escribe(&mut self, bytes: &[u8]) {
        let limite = usize::try_from(self.limite).unwrap_or(usize::MAX);
        let bytes = &bytes[..bytes.len().min(limite)];
        let largo = bytes.len() as u64;
        if self.tamano > 0 && self.tamano + largo > self.limite {
            self.rota();
        }
        if let Some(fichero) = self.fichero.as_mut() {
            if fichero.write_all(bytes).is_ok() {
                self.tamano += largo;
            }
        }
    }

    /// Pasa el fichero actual a `.1` (reemplazando la copia anterior) y empieza uno vacío. Si no se
    /// puede renombrar, se trunca: la cota se cumple igualmente.
    fn rota(&mut self) {
        self.fichero = None;
        let _ = fs::remove_file(&self.viejo);
        let _ = fs::rename(&self.actual, &self.viejo);
        self.fichero = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.actual)
            .ok();
        self.tamano = 0;
    }
}

/// Escritor del registro. Cada evento se acumula en memoria y se vuelca de una vez al cerrarse.
#[derive(Clone)]
pub struct Escritor {
    estado: Arc<Mutex<Estado>>,
}

impl Escritor {
    /// Abre (o crea) `ruta` en modo añadir. Falla si no se puede crear el directorio o el fichero.
    pub fn abre(ruta: &Path, limite: u64) -> io::Result<Escritor> {
        if let Some(padre) = ruta.parent() {
            fs::create_dir_all(padre)?;
        }
        let fichero = OpenOptions::new().create(true).append(true).open(ruta)?;
        let tamano = fichero.metadata()?.len();
        let mut viejo = ruta.as_os_str().to_owned();
        viejo.push(".1");
        Ok(Escritor {
            estado: Arc::new(Mutex::new(Estado {
                actual: ruta.to_path_buf(),
                viejo: PathBuf::from(viejo),
                limite,
                fichero: Some(fichero),
                tamano,
            })),
        })
    }
}

/// Búfer de un evento: se vuelca al fichero al soltarlo.
pub struct Entrada {
    buf: Vec<u8>,
    estado: Arc<Mutex<Estado>>,
}

impl Write for Entrada {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Entrada {
    fn drop(&mut self) {
        if let Ok(mut estado) = self.estado.lock() {
            estado.escribe(&self.buf);
        }
    }
}

impl<'a> MakeWriter<'a> for Escritor {
    type Writer = Entrada;

    fn make_writer(&'a self) -> Entrada {
        Entrada {
            buf: Vec::new(),
            estado: Arc::clone(&self.estado),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::EnvFilter;

    fn temporal() -> tempfile::TempDir {
        tempfile::tempdir().expect("directorio temporal")
    }

    /// Ejecuta `emite` con un suscriptor local (sin tocar el global del proceso de pruebas).
    fn con_filtro(ruta: &Path, limite: u64, filtro: &str, emite: impl FnOnce()) {
        let escritor = Escritor::abre(ruta, limite).expect("el fichero de prueba se abre");
        let sub = suscriptor(escritor, EnvFilter::new(filtro));
        tracing::subscriber::with_default(sub, emite);
    }

    #[test]
    fn sin_rust_log_no_se_instala_ni_se_crea_nada() {
        let dir = temporal();
        let datos = dir.path().join("datos");
        assert!(instala_con("cliente", &datos, None).is_none());
        assert!(instala_con("cliente", &datos, Some("   ")).is_none());
        assert!(
            !datos.exists(),
            "sin RUST_LOG no debe crearse el directorio"
        );
    }

    #[test]
    fn el_filtro_sale_de_rust_log_y_se_ignora_si_esta_vacio() {
        assert!(filtro_desde(None).is_none());
        assert!(filtro_desde(Some("")).is_none());
        assert!(filtro_desde(Some("output_filter=info")).is_some());
    }

    #[test]
    fn solo_llegan_los_eventos_del_destino_pedido() {
        let dir = temporal();
        let ruta = dir.path().join("loomcoding-prueba.log");
        con_filtro(&ruta, MAX_LOG_BYTES, "output_filter=info", || {
            tracing::info!(target: "output_filter", regla = "general", "salida filtrada");
            tracing::info!(target: "odd", "medicion descartada");
        });
        let texto = fs::read_to_string(&ruta).unwrap();
        assert!(texto.contains("salida filtrada"), "{texto}");
        assert!(texto.contains("regla=\"general\""), "{texto}");
        assert!(texto.contains("output_filter"), "{texto}");
        assert!(!texto.contains("medicion descartada"), "{texto}");
    }

    #[test]
    fn al_superar_la_cota_rota_y_el_actual_queda_por_debajo() {
        let dir = temporal();
        let ruta = dir.path().join("loomcoding-prueba.log");
        const COTA: u64 = 300;
        con_filtro(&ruta, COTA, "info", || {
            for i in 0..20 {
                tracing::info!("linea de prueba numero {i:03}");
            }
        });
        let viejo = dir.path().join("loomcoding-prueba.log.1");
        assert!(viejo.exists(), "debe existir la copia rotada");
        let actual = fs::metadata(&ruta).unwrap().len();
        assert!(actual <= COTA, "el fichero actual mide {actual} bytes");
        assert!(fs::metadata(&viejo).unwrap().len() <= COTA);
        let texto = fs::read_to_string(&ruta).unwrap();
        assert!(
            texto.contains("numero 019"),
            "el último evento debe quedar: {texto}"
        );
    }

    #[test]
    fn un_directorio_no_escribible_no_provoca_error_ni_panico() {
        let dir = temporal();
        // Un fichero donde debería haber un directorio: crear `logs` dentro falla.
        let bloqueo = dir.path().join("bloqueo");
        fs::write(&bloqueo, b"no soy un directorio").unwrap();
        assert!(instala_con("daemon", &bloqueo, Some("info")).is_none());
        assert!(Escritor::abre(&bloqueo.join("logs").join("x.log"), 10).is_err());
    }
}
