//! Pruebas de la regla `packages` con salidas reales de `npm install` y `pnpm install` (R3, R5, R6,
//! R7). Las capturas están en `fixtures/` y se sacaron de proyectos de juguete con dependencias
//! deprecadas; la ruta del almacén de pnpm está anonimizada.

use super::*;
use serde_json::json;

const NPM: &str = include_str!("fixtures/npm_install_deprecaciones.txt");
const NPM_DOS_VERSIONES: &str =
    include_str!("fixtures/npm_install_har_validator_dos_versiones.txt");
const PNPM: &str = include_str!("fixtures/pnpm_install_progreso.txt");
const NPM_ERROR: &str = "npm error code ERESOLVE\nnpm error ERESOLVE unable to resolve dependency tree\nnpm error Found: react@18.2.0\n";

/// Filtro encendido, como lo deja `[output_filter] enabled = true`.
fn activo() -> FilterConfig {
    FilterConfig::from_toml("[output_filter]\nenabled = true\n").unwrap()
}

/// Volcado temporal que sobrevive al proceso de pruebas.
fn volcado() -> ToolSpill {
    ToolSpill::new(tempfile::tempdir().unwrap().keep())
}

/// Filtra una salida de `bash` con el comando dado.
fn bash_paquetes(comando: &str, texto: &str) -> Filtered {
    apply(
        "c1",
        "bash",
        &json!({"command": comando}),
        &ToolOutput::ok(texto),
        &activo(),
        Some(&volcado()),
    )
}

/// Propiedad R3: toda línea con marca de fallo o aviso sobrevive intacta en la salida filtrada.
fn tiene_senal(linea: &str) -> bool {
    let minusculas = linea.to_ascii_lowercase();
    ["error", "failed", "panic", "warning:"]
        .iter()
        .any(|marca| minusculas.contains(marca))
}

/// Barra de progreso de pnpm: una línea hecha solo de `+` o de `-`.
fn es_barra(linea: &str) -> bool {
    let recortada = linea.trim();
    recortada.len() >= 3 && recortada.chars().all(|c| c == '+' || c == '-')
}

#[test]
fn npm_sin_ruido_pasa_sin_tocar() {
    let salida = bash_paquetes("npm install", NPM);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, NPM);
}

#[test]
fn deprecacion_repetida_con_el_mismo_texto_se_cuenta_una_vez() {
    // npm 11 ya deduplica sus avisos. La repetición se simula con una línea real de otra captura
    // del mismo paquete en otra versión: mismo nombre y mismo mensaje.
    let segunda = NPM
        .lines()
        .find(|linea| linea.contains("har-validator@5.1.5"))
        .unwrap();
    let texto = format!("{segunda}\n{NPM_DOS_VERSIONES}");
    let salida = bash_paquetes("npm install", &texto);

    assert_eq!(salida.rule, Some("packages"), "{}", salida.content);
    assert!(
        salida.content.contains(
            "npm warn deprecated har-validator@5.1.5: this library is no longer supported ×2"
        ),
        "{}",
        salida.content
    );
    assert_eq!(salida.content.matches("har-validator").count(), 1);
    assert_eq!(salida.omitted_lines, 1);
}

#[test]
fn pnpm_conserva_solo_el_ultimo_progreso_y_quita_las_barras() {
    let salida = bash_paquetes("pnpm install", PNPM);

    assert_eq!(salida.rule, Some("packages"), "{}", salida.content);
    assert!(
        salida
            .content
            .contains("Progress: resolved 58, reused 9, downloaded 49, added 58, done"),
        "{}",
        salida.content
    );
    assert!(
        !salida.content.contains("Progress: resolved 1,"),
        "{}",
        salida.content
    );
    assert!(!salida.content.contains("++++"), "{}", salida.content);
    assert!(
        salida.content.contains("[WARN] deprecated request@2.88.2"),
        "{}",
        salida.content
    );
    assert!(
        salida
            .content
            .contains("[WARN] 2 deprecated subdependencies found"),
        "{}",
        salida.content
    );
    let progreso = PNPM.lines().filter(|l| l.starts_with("Progress: ")).count();
    let barras = PNPM.lines().filter(|l| es_barra(l)).count();
    assert_eq!(salida.omitted_lines, progreso - 1 + barras);
}

#[test]
fn una_linea_de_deprecacion_con_marca_de_senal_nunca_se_deduplica() {
    // R3: un aviso que menciona un error no es ruido, aunque se repita con el mismo mensaje.
    let texto = "npm warn deprecated libreria-vieja@1.0.0: error conocido en la version antigua\nnpm warn deprecated libreria-vieja@1.0.1: error conocido en la version antigua\nadded 2 packages in 1s\n";
    let salida = bash_paquetes("npm install", texto);

    assert_eq!(salida.omitted_lines, 0, "{}", salida.content);
    assert_eq!(salida.content.matches("libreria-vieja").count(), 2);
}

#[test]
fn propiedad_ninguna_linea_con_senal_desaparece_en_las_capturas_de_paquetes() {
    let doble = format!("{NPM_ERROR}{NPM_DOS_VERSIONES}");
    let capturas = [
        ("npm install", NPM),
        ("npm install", NPM_DOS_VERSIONES),
        ("npm install", doble.as_str()),
        ("pnpm install", PNPM),
    ];
    for (comando, texto) in capturas {
        let salida = bash_paquetes(comando, texto);
        for linea in texto.lines().filter(|l| tiene_senal(l)) {
            assert!(
                salida.content.contains(linea),
                "{comando}: se perdió «{linea}»\n{}",
                salida.content
            );
        }
    }
}

#[test]
fn npm_con_error_pasa_sin_regla_packages() {
    let salida = bash_paquetes("npm install", NPM_ERROR);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, NPM_ERROR);
}

#[test]
fn solo_install_e_i_tienen_regla() {
    assert_ne!(bash_paquetes("npm ci", NPM).rule, Some("packages"));
    assert_ne!(bash_paquetes("npm run build", NPM).rule, Some("packages"));
    assert_eq!(bash_paquetes("npm i", NPM_DOS_VERSIONES).rule, None);
    assert_ne!(bash_paquetes("pnpm add react", PNPM).rule, Some("packages"));
}

#[test]
fn con_tuberias_o_encadenados_no_hay_regla_packages() {
    assert_ne!(
        bash_paquetes("npm install | tail -5", NPM_DOS_VERSIONES).rule,
        Some("packages")
    );
    assert_ne!(
        bash_paquetes("npm install && npm test", NPM_DOS_VERSIONES).rule,
        Some("packages")
    );
    assert_ne!(
        bash_paquetes("pnpm install 2>&1 | cat", PNPM).rule,
        Some("packages")
    );
    assert_eq!(
        bash_paquetes("pnpm install 2>&1", PNPM).rule,
        Some("packages")
    );
}

#[test]
fn salida_json_o_idioma_distinto_pasa_sin_tocar() {
    let json_npm = "{\"added\": 58, \"audited\": 59}\n";
    let espanol = "Instalados 3 paquetes en 1s\nnpm warn deprecated paquete@1.0.0: obsoleto\n";
    for texto in [json_npm, espanol] {
        let salida = bash_paquetes("npm install --json", texto);
        assert_eq!(salida.rule, None, "{}", salida.content);
        assert_eq!(salida.content, texto);
    }
}
