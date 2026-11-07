//! Análisis mínimo del comando de `bash` para elegir regla (R6). Solo un comando simple tiene regla:
//! cualquier tubería, encadenado, sustitución o redirección, salvo `2>&1` (que no encadena nada),
//! deja la salida sin tocar.

/// Caracteres que encadenan, redirigen o sustituyen comandos. Ante cualquiera, no hay regla.
const ENCADENAMIENTO: [char; 9] = ['|', '&', ';', '\n', '`', '$', '>', '<', '('];

/// Palabras de un comando simple, sin la redirección `2>&1`. Devuelve `None` si el comando encadena,
/// sustituye o redirige algo.
pub fn tokens(comando: &str) -> Option<Vec<&str>> {
    if comando
        .replace("2>&1", "")
        .contains(|c| ENCADENAMIENTO.contains(&c))
    {
        return None;
    }
    Some(
        comando
            .split_whitespace()
            .filter(|token| *token != "2>&1")
            .collect(),
    )
}
