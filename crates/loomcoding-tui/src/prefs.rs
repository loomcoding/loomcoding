//! Preferencias de la interfaz que sobreviven entre sesiones (el tema y la
//! captura del ratón).
//!
//! Este fichero pertenece a la TUI y no forma parte de `LoomcodingConfig`. Su
//! formato es un TOML mínimo: `theme = "<clave>"` y `mouse = true|false`. Al
//! guardar se conservan las demás líneas del fichero tal cual, sin
//! interpretarlas; por eso no se usa un parser TOML completo ni se añade
//! ninguna dependencia nueva.

use std::io;
use std::path::Path;

use loomcoding_cloth::state::StyleToken;
use loomcoding_cloth::widgets::matrix_rain::{GlyphSet, MatrixConfig};
use loomcoding_cloth::widgets::{LayoutStyle, ThemePreset};

const THEME_KEY: &str = "theme";
const MOUSE_KEY: &str = "mouse";
const SPINNER_KEY: &str = "spinner";
const LAYOUT_KEY: &str = "layout";
const MATRIX_GLYPHS_KEY: &str = "matrix_glyphs";
const MATRIX_SPEED_KEY: &str = "matrix_speed";
const MATRIX_DENSITY_KEY: &str = "matrix_density";
const MATRIX_LENGTH_KEY: &str = "matrix_length";
const MATRIX_COLOR_KEY: &str = "matrix_color";

/// Identificador estable y en minúsculas de cada preset, tal como se escribe en el fichero.
fn key(preset: ThemePreset) -> &'static str {
    match preset {
        ThemePreset::Cloth => "cloth",
        ThemePreset::Martty => "martty",
        ThemePreset::Mindfork => "mindfork",
        ThemePreset::Pawan => "pawan",
        ThemePreset::Crush => "crush",
        ThemePreset::MastraCode => "mastracode",
        ThemePreset::Loomcoding => "loomcoding",
    }
}

fn from_key(value: &str) -> Option<ThemePreset> {
    ThemePreset::ALL
        .into_iter()
        .find(|preset| key(*preset) == value)
}

/// Indica si una línea asigna la clave `name` (`name = ...`).
fn is_assignment_of(line: &str, name: &str) -> bool {
    line.split_once('=')
        .is_some_and(|(left, _)| left.trim() == name)
}

/// Lee el valor crudo (sin espacios) de la primera línea que asigna `name` y
/// lo interpreta con `parse`. Un fichero inexistente, ilegible o con un valor
/// no reconocido produce `None`; nunca entra en pánico.
fn load_value<T>(path: &Path, name: &str, parse: impl Fn(&str) -> Option<T>) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().find_map(|line| {
        let (left, value) = line.split_once('=')?;
        if left.trim() != name {
            return None;
        }
        parse(value.trim())
    })
}

/// Escribe `name = value`, sustituyendo cualquier línea previa que asigne
/// `name` y conservando el resto de líneas. Crea el directorio padre si no
/// existe. `value` ya debe venir formateado (con comillas si es texto).
fn save_value(path: &Path, name: &str, value: &str) -> io::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let mut content = String::new();
    for line in existing.lines() {
        if !is_assignment_of(line, name) {
            content.push_str(line);
            content.push('\n');
        }
    }
    content.push_str(&format!("{name} = {value}\n"));

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, content)
}

/// Lee el tema guardado. Un fichero inexistente, ilegible, corrupto o con una
/// clave desconocida produce `None`; nunca entra en pánico.
pub fn load_theme(path: &Path) -> Option<ThemePreset> {
    load_value(path, THEME_KEY, |value| {
        from_key(value.strip_prefix('"')?.strip_suffix('"')?)
    })
}

/// Guarda el tema, conservando el resto de líneas del fichero.
pub fn save_theme(path: &Path, preset: ThemePreset) -> io::Result<()> {
    save_value(path, THEME_KEY, &format!("\"{}\"", key(preset)))
}

/// Lee la captura del ratón guardada (`mouse = true|false`). Cualquier otro
/// valor, o la ausencia de la clave, produce `None`.
pub fn load_mouse(path: &Path) -> Option<bool> {
    load_value(path, MOUSE_KEY, |value| match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    })
}

/// Guarda la captura del ratón, conservando el resto de líneas del fichero.
pub fn save_mouse(path: &Path, on: bool) -> io::Result<()> {
    save_value(path, MOUSE_KEY, if on { "true" } else { "false" })
}

/// Lee el spinner guardado: un nombre de estilo, `"random"` o `"crush"`.
///
/// Cualquier otro valor produce `None` y se queda el de por defecto.
pub fn load_spinner(path: &Path) -> Option<String> {
    load_value(path, SPINNER_KEY, |value| {
        Some(value.strip_prefix('"')?.strip_suffix('"')?.to_string())
    })
}

/// Guarda el spinner elegido, conservando el resto de líneas del fichero.
pub fn save_spinner(path: &Path, name: &str) -> io::Result<()> {
    save_value(path, SPINNER_KEY, &format!("\"{name}\""))
}

/// Lee la disposición guardada (`layout = "rail"`, `"minimal"`, ...). Es una
/// preferencia independiente del color: un nombre desconocido produce `None`.
pub fn load_layout(path: &Path) -> Option<LayoutStyle> {
    load_value(path, LAYOUT_KEY, |value| {
        LayoutStyle::from_name(value.strip_prefix('"')?.strip_suffix('"')?)
    })
}

/// Guarda la disposición, conservando el resto de líneas del fichero.
pub fn save_layout(path: &Path, layout: LayoutStyle) -> io::Result<()> {
    save_value(path, LAYOUT_KEY, &format!("\"{}\"", layout.name()))
}

/// Aplica sobre `base` (la config del tema) los ajustes de lluvia que el
/// usuario haya guardado. Cada campo es independiente: lo que falte en el
/// fichero conserva el valor del tema.
pub fn load_matrix(path: &Path, base: &MatrixConfig) -> MatrixConfig {
    let mut config = base.clone();
    if let Some(name) = load_value(path, MATRIX_GLYPHS_KEY, |v| {
        Some(v.strip_prefix('"')?.strip_suffix('"')?.to_string())
    }) {
        if let Some(glyphs) = glyphs_from_name(&name) {
            config.glyphs = glyphs;
        }
    }
    if let Some(speed) = load_value(path, MATRIX_SPEED_KEY, |v| v.parse::<f32>().ok()) {
        config.speed = speed;
    }
    if let Some(density) = load_value(path, MATRIX_DENSITY_KEY, |v| v.parse::<u8>().ok()) {
        config.density = density;
    }
    if let Some(length) = load_value(path, MATRIX_LENGTH_KEY, |v| v.parse::<u16>().ok()) {
        config.length = length;
    }
    if let Some(name) = load_value(path, MATRIX_COLOR_KEY, |v| {
        Some(v.strip_prefix('"')?.strip_suffix('"')?.to_string())
    }) {
        if let Some(token) = StyleToken::from_key(&name) {
            config.color = token;
        }
    }
    config.clamped()
}

/// Guarda la config de la lluvia, conservando el resto de líneas del fichero.
pub fn save_matrix(path: &Path, config: &MatrixConfig) -> io::Result<()> {
    save_value(
        path,
        MATRIX_GLYPHS_KEY,
        &format!("\"{}\"", config.glyphs.preset_name()),
    )?;
    save_value(path, MATRIX_SPEED_KEY, &format!("{:.2}", config.speed))?;
    save_value(path, MATRIX_DENSITY_KEY, &config.density.to_string())?;
    save_value(path, MATRIX_LENGTH_KEY, &config.length.to_string())?;
    save_value(
        path,
        MATRIX_COLOR_KEY,
        &format!("\"{}\"", config.color.key()),
    )
}

/// Resuelve un nombre de preset de glifos. Un nombre desconocido se interpreta
/// como juego propio; si no es válido, se ignora y se conserva el del tema.
fn glyphs_from_name(name: &str) -> Option<GlyphSet> {
    match name.trim().to_ascii_lowercase().as_str() {
        "katakana" => Some(GlyphSet::katakana()),
        "emoji" => Some(GlyphSet::emoji()),
        "binario" | "binary" => Some(GlyphSet::binary()),
        "ascii" => Some(GlyphSet::ascii()),
        "mixto" | "mixed" => Some(GlyphSet::mixed()),
        other => GlyphSet::custom(other).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_round_trips_and_coexists_with_the_theme() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        save_theme(&path, ThemePreset::Crush).unwrap();
        save_spinner(&path, "random").unwrap();
        assert_eq!(load_spinner(&path).as_deref(), Some("random"));
        assert_eq!(load_theme(&path), Some(ThemePreset::Crush));

        save_spinner(&path, "crush").unwrap();
        assert_eq!(load_spinner(&path).as_deref(), Some("crush"));
        assert_eq!(load_theme(&path), Some(ThemePreset::Crush));
    }

    #[test]
    fn a_missing_spinner_key_keeps_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        save_theme(&path, ThemePreset::Crush).unwrap();
        assert_eq!(load_spinner(&path), None);
    }

    #[test]
    fn theme_round_trips_for_every_preset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        for preset in ThemePreset::ALL {
            save_theme(&path, preset).unwrap();
            assert_eq!(load_theme(&path), Some(preset), "preset {preset:?}");
        }
    }

    #[test]
    fn loomcoding_theme_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        save_theme(&path, ThemePreset::Loomcoding).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "theme = \"loomcoding\"\n");
        assert_eq!(load_theme(&path), Some(ThemePreset::Loomcoding));
    }

    #[test]
    fn missing_or_corrupt_file_loads_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_theme(&dir.path().join("no-existe.toml")), None);

        let corrupt = dir.path().join("corrupt.toml");
        std::fs::write(&corrupt, [0xff, 0xfe, 0x00, b'=', b'"']).unwrap();
        assert_eq!(load_theme(&corrupt), None);

        let garbage = dir.path().join("garbage.toml");
        std::fs::write(&garbage, "theme = \nno es toml \"\"\"").unwrap();
        assert_eq!(load_theme(&garbage), None);
    }

    #[test]
    fn unknown_key_loads_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        std::fs::write(&path, "theme = \"solarized\"\n").unwrap();
        assert_eq!(load_theme(&path), None);
    }

    #[test]
    fn save_creates_the_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anidado").join("ui.toml");
        save_theme(&path, ThemePreset::Crush).unwrap();
        assert_eq!(load_theme(&path), Some(ThemePreset::Crush));
    }

    #[test]
    fn save_keeps_other_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        std::fs::write(
            &path,
            "# preferencias\nmodo = \"denso\"\ntheme = \"pawan\"\n",
        )
        .unwrap();
        save_theme(&path, ThemePreset::Crush).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# preferencias\n"));
        assert!(text.contains("modo = \"denso\"\n"));
        assert_eq!(text.matches("theme").count(), 1);
        assert_eq!(load_theme(&path), Some(ThemePreset::Crush));
    }

    #[test]
    fn mouse_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        for on in [true, false, true] {
            save_mouse(&path, on).unwrap();
            assert_eq!(load_mouse(&path), Some(on), "mouse {on}");
        }
    }

    #[test]
    fn mouse_and_theme_keep_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        save_theme(&path, ThemePreset::Pawan).unwrap();
        save_mouse(&path, false).unwrap();
        assert_eq!(load_theme(&path), Some(ThemePreset::Pawan));
        assert_eq!(load_mouse(&path), Some(false));

        save_theme(&path, ThemePreset::Crush).unwrap();
        assert_eq!(load_mouse(&path), Some(false));
        assert_eq!(load_theme(&path), Some(ThemePreset::Crush));

        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("mouse").count(), 1);
        assert_eq!(text.matches("theme").count(), 1);
    }

    #[test]
    fn matrix_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        let config = MatrixConfig {
            glyphs: GlyphSet::emoji(),
            speed: 1.25,
            density: 40,
            length: 12,
            color: StyleToken::Accent,
        };
        save_matrix(&path, &config).unwrap();
        assert_eq!(load_matrix(&path, &MatrixConfig::default()), config);
    }

    #[test]
    fn matrix_missing_file_keeps_theme_values() {
        let dir = tempfile::tempdir().unwrap();
        let base = MatrixConfig {
            glyphs: GlyphSet::binary(),
            speed: 2.0,
            density: 50,
            length: 9,
            color: StyleToken::Warning,
        };
        assert_eq!(load_matrix(&dir.path().join("no-existe.toml"), &base), base);
    }

    #[test]
    fn matrix_partial_file_overrides_only_present_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        std::fs::write(&path, "matrix_speed = 3.00\n").unwrap();
        let base = MatrixConfig {
            glyphs: GlyphSet::binary(),
            speed: 0.5,
            density: 50,
            length: 9,
            color: StyleToken::Warning,
        };
        let loaded = load_matrix(&path, &base);
        assert_eq!(loaded.speed, 3.0);
        // El resto conserva los valores del tema.
        assert_eq!(loaded.glyphs, GlyphSet::binary());
        assert_eq!(loaded.density, 50);
        assert_eq!(loaded.length, 9);
        assert_eq!(loaded.color, StyleToken::Warning);
    }

    #[test]
    fn matrix_garbage_values_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        std::fs::write(
            &path,
            "matrix_speed = \"rapido\"\nmatrix_density = -5\nmatrix_color = \"no-existe\"\n",
        )
        .unwrap();
        let base = MatrixConfig::default();
        assert_eq!(load_matrix(&path, &base), base);
    }

    #[test]
    fn matrix_out_of_range_values_are_clamped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        std::fs::write(&path, "matrix_speed = 999.00\nmatrix_density = 250\n").unwrap();
        let loaded = load_matrix(&path, &MatrixConfig::default());
        assert_eq!(loaded.speed, 4.0);
        assert_eq!(loaded.density, 100);
    }

    #[test]
    fn matrix_and_theme_keep_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        save_theme(&path, ThemePreset::Crush).unwrap();
        save_matrix(&path, &MatrixConfig::default()).unwrap();
        assert_eq!(load_theme(&path), Some(ThemePreset::Crush));
        assert_eq!(
            load_matrix(&path, &MatrixConfig::default()),
            MatrixConfig::default()
        );

        save_theme(&path, ThemePreset::Pawan).unwrap();
        assert_eq!(load_theme(&path), Some(ThemePreset::Pawan));
        assert_eq!(
            load_matrix(&path, &MatrixConfig::default()),
            MatrixConfig::default()
        );
    }

    #[test]
    fn layout_survives_a_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.toml");
        save_theme(&path, ThemePreset::Crush).unwrap();
        save_spinner(&path, "random").unwrap();
        for layout in [
            LayoutStyle::Minimal,
            LayoutStyle::Cards,
            LayoutStyle::ContextRail,
            LayoutStyle::Workspace,
        ] {
            save_layout(&path, layout).unwrap();
            assert_eq!(load_layout(&path), Some(layout), "layout {layout:?}");
        }
        // Guardar el layout no toca el tema ni el spinner, y no duplica la clave.
        save_layout(&path, LayoutStyle::Cards).unwrap();
        assert_eq!(load_theme(&path), Some(ThemePreset::Crush));
        assert_eq!(load_spinner(&path).as_deref(), Some("random"));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("layout").count(), 1);
    }

    #[test]
    fn layout_missing_or_unknown_loads_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_layout(&dir.path().join("no-existe.toml")), None);

        let path = dir.path().join("ui.toml");
        std::fs::write(&path, "layout = \"tabla\"\n").unwrap();
        assert_eq!(load_layout(&path), None);
    }

    #[test]
    fn mouse_missing_or_garbage_loads_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_mouse(&dir.path().join("no-existe.toml")), None);

        for (name, content) in [
            ("vacio.toml", "mouse = \n"),
            ("texto.toml", "mouse = \"true\"\n"),
            ("mayus.toml", "mouse = True\n"),
            ("numero.toml", "mouse = 1\n"),
            ("otra.toml", "mouse_mode = true\n"),
        ] {
            let path = dir.path().join(name);
            std::fs::write(&path, content).unwrap();
            assert_eq!(load_mouse(&path), None, "contenido {content:?}");
        }
    }
}
