//! Perfil de usuario de Loomcoding. En Windows `dirs::home_dir()` ignora `USERPROFILE`, así que
//! `LOOMCODING_HOME` permite aislar el perfil (sesiones, token, tubería del daemon) sin tocar
//! el del usuario; lo usan las pruebas E2E.

use std::ffi::OsString;
use std::path::PathBuf;

pub const HOME_ENV: &str = "LOOMCODING_HOME";

fn resolve(over: Option<OsString>, fallback: Option<PathBuf>) -> Option<PathBuf> {
    over.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or(fallback)
}

/// Directorio de perfil: `LOOMCODING_HOME` si está definido y no vacío; si no, el del sistema.
pub fn user_home() -> Option<PathBuf> {
    resolve(std::env::var_os(HOME_ENV), dirs::home_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_wins_over_the_system_home() {
        let got = resolve(Some("X".into()), Some(PathBuf::from("Y")));
        assert_eq!(got, Some(PathBuf::from("X")));
    }

    #[test]
    fn an_empty_override_is_ignored() {
        let got = resolve(Some("".into()), Some(PathBuf::from("Y")));
        assert_eq!(got, Some(PathBuf::from("Y")));
    }

    #[test]
    fn without_override_or_system_home_there_is_none() {
        assert_eq!(resolve(None, None), None);
    }
}
