//! Operaciones que escriben o leen el documento en disco: create, reopen, block, log y read.

use super::*;

#[tokio::test]
async fn create_escribe_el_documento_con_tareas_numeradas() {
    let e = entorno().await;
    let out = e
        .llamar(json!({
            "op": "create",
            "feature": "login-social",
            "objective": "Entrar con Google",
            "tasks": ["Modelo de sesión", "Pantalla de login"],
        }))
        .await;
    assert!(!out.is_error, "{}", out.content);
    let md = e.leer("login-social");
    assert!(md.contains("- [ ] T1 Modelo de sesión"), "{md}");
    assert!(md.contains("- [ ] T2 Pantalla de login"), "{md}");
    let doc = parsear(&md);
    assert_eq!(doc.objective, "Entrar con Google");
    assert_eq!(
        e.estado.active_feature(e.sesion).as_deref(),
        Some("login-social")
    );
}

#[tokio::test]
async fn create_crea_la_carpeta_si_falta_y_no_deja_temporales() {
    let e = entorno().await;
    e.crear_basico("f", &["uno"]).await;
    let entradas: Vec<String> = std::fs::read_dir(e.proyecto.path().join(".loomcoding/odd"))
        .unwrap()
        .map(|d| d.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entradas, vec!["f.md".to_string()]);
}

#[tokio::test]
async fn create_sobre_un_documento_existente_se_rechaza_y_no_lo_toca() {
    let e = entorno().await;
    e.crear_basico("f", &["uno"]).await;
    let antes = e.leer("f");
    let out = e
        .llamar(json!({"op": "create", "feature": "f", "objective": "otro", "tasks": ["dos"]}))
        .await;
    assert!(out.is_error);
    assert!(out.content.contains("ya existe"), "{}", out.content);
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn dos_create_concurrentes_del_mismo_nombre_solo_gana_uno() {
    let e = entorno().await;
    let input = json!({"op": "create", "feature": "carrera", "objective": "o", "tasks": ["t"]});
    let (a, b) = tokio::join!(e.llamar(input.clone()), e.llamar(input));
    assert_ne!(a.is_error, b.is_error, "{} / {}", a.content, b.content);
}

#[tokio::test]
async fn create_rechaza_textos_con_separadores_del_formato() {
    let e = entorno().await;
    let casos = [
        json!({"op":"create","feature":"f","objective":"Objetivo — evidencia: falso","tasks":["t"]}),
        json!({"op":"create","feature":"f","objective":"o","tasks":["hago x — evidencia: y"]}),
        json!({"op":"create","feature":"f","objective":"o","tasks":["hago x — bloqueada: z"]}),
    ];
    for input in casos {
        let out = e.llamar(input.clone()).await;
        assert!(out.is_error, "debía rechazar {input}");
        assert!(out.content.contains("separador"), "{}", out.content);
    }
    assert!(!e.ruta("f").exists(), "ningún caso debe escribir nada");
}

#[tokio::test]
async fn una_tarea_de_mas_de_300_bytes_se_rechaza_y_una_de_300_se_acepta() {
    let e = entorno().await;
    let largo = "a".repeat(301);
    let out = e
        .llamar(json!({"op":"create","feature":"f","objective":"o","tasks":[largo]}))
        .await;
    assert!(out.is_error);
    assert!(out.content.contains("300"), "{}", out.content);
    assert!(!e.ruta("f").exists());

    let justo = "a".repeat(300);
    let out = e
        .llamar(json!({"op":"create","feature":"f","objective":"o","tasks":[justo]}))
        .await;
    assert!(!out.is_error, "{}", out.content);
}

#[tokio::test]
async fn los_saltos_de_linea_en_textos_del_modelo_se_rechazan() {
    let e = entorno().await;
    let out = e
        .llamar(json!({"op":"create","feature":"f","objective":"o","tasks":["uno\ndos"]}))
        .await;
    assert!(out.is_error);
    assert!(out.content.contains("salto de línea"), "{}", out.content);
    assert!(!e.ruta("f").exists());
}

#[tokio::test]
async fn bloquear_y_reabrir_quedan_en_el_documento_con_motivo() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno", "Tarea dos"]).await;

    let out = e
        .llamar(json!({"op":"block","feature":"f","id":"T1","reason":"falta la clave"}))
        .await;
    assert!(!out.is_error, "{}", out.content);
    let md = e.leer("f");
    assert!(
        md.contains("- [!] T1 Tarea uno — bloqueada: falta la clave"),
        "{md}"
    );

    let out = e
        .llamar(json!({"op":"reopen","feature":"f","id":"T1","reason":"clave recibida"}))
        .await;
    assert!(!out.is_error, "{}", out.content);
    let md = e.leer("f");
    assert!(md.contains("- [ ] T1 Tarea uno\n"), "{md}");
    assert!(md.contains("T1 reabierta: clave recibida"), "{md}");
}

#[tokio::test]
async fn bloquear_y_reabrir_validan_motivo_estado_e_identificador() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let antes = e.leer("f");

    let sin_motivo = e
        .llamar(json!({"op":"block","feature":"f","id":"T1","reason":"  "}))
        .await;
    assert!(sin_motivo.is_error);
    let reabrir_pendiente = e
        .llamar(json!({"op":"reopen","feature":"f","id":"T1","reason":"nada"}))
        .await;
    assert!(reabrir_pendiente.is_error, "{}", reabrir_pendiente.content);
    let desconocida = e
        .llamar(json!({"op":"block","feature":"f","id":"T9","reason":"x"}))
        .await;
    assert!(desconocida.is_error);
    assert!(
        desconocida.content.contains("T9"),
        "{}",
        desconocida.content
    );
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn el_motivo_de_bloqueo_con_separador_se_rechaza() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;
    let antes = e.leer("f");
    let out = e
        .llamar(json!({"op":"block","feature":"f","id":"T1","reason":"x — bloqueada: y"}))
        .await;
    assert!(out.is_error);
    assert!(out.content.contains("separador"), "{}", out.content);
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn log_anade_una_linea_al_registro_y_rechaza_separadores_y_excesos() {
    let e = entorno().await;
    e.crear_basico("f", &["Tarea uno"]).await;

    let out = e
        .llamar(json!({"op":"log","feature":"f","text":"Explorado el módulo de sesión"}))
        .await;
    assert!(!out.is_error, "{}", out.content);
    assert!(parsear(&e.leer("f"))
        .log
        .iter()
        .any(|l| l == "Explorado el módulo de sesión"));

    let antes = e.leer("f");
    let con_separador = e
        .llamar(json!({"op":"log","feature":"f","text":"nota — evidencia: x"}))
        .await;
    assert!(con_separador.is_error);
    let demasiado = e
        .llamar(json!({"op":"log","feature":"f","text":"l".repeat(301)}))
        .await;
    assert!(demasiado.is_error);
    assert_eq!(e.leer("f"), antes);
}

#[tokio::test]
async fn read_devuelve_el_documento_y_falla_si_no_existe() {
    let e = entorno().await;
    let sin = e.llamar(json!({"op":"read","feature":"f"})).await;
    assert!(sin.is_error);
    assert!(sin.content.contains("no existe"), "{}", sin.content);

    e.crear_basico("f", &["Tarea uno"]).await;
    let out = e.llamar(json!({"op":"read","feature":"f"})).await;
    assert!(!out.is_error, "{}", out.content);
    assert!(
        out.content.contains("- [ ] T1 Tarea uno"),
        "{}",
        out.content
    );
}

#[tokio::test]
async fn read_queda_acotado_aunque_el_documento_ocupe_el_maximo() {
    let e = entorno().await;
    // Documento legal de casi 32 KiB: 100 líneas de registro de 300 bytes.
    let mut md = String::from("# f\n\n## Tareas\n\n- [ ] T1 tarea\n\n## Registro\n\n");
    for _ in 0..100 {
        md.push_str(&format!("- {}\n", "r".repeat(300)));
    }
    assert!(md.len() <= crate::odd::doc::MAX_DOC_BYTES);
    std::fs::create_dir_all(e.ruta("f").parent().unwrap()).unwrap();
    std::fs::write(e.ruta("f"), &md).unwrap();

    let out = e.llamar(json!({"op":"read","feature":"f"})).await;
    assert!(!out.is_error, "{}", out.content);
    assert!(
        out.content.len() <= loomcoding_core::tools::MAX_TOOL_OUTPUT_BYTES + 100,
        "{} bytes",
        out.content.len()
    );
}

#[tokio::test]
async fn documento_roto_da_error_claro_y_no_se_sobrescribe() {
    let e = entorno().await;
    std::fs::create_dir_all(e.ruta("f").parent().unwrap()).unwrap();
    std::fs::write(e.ruta("f"), "hola sin cabecera\n").unwrap();

    let read = e.llamar(json!({"op":"read","feature":"f"})).await;
    assert!(read.is_error);
    assert!(read.content.contains("cabecera"), "{}", read.content);

    let log = e
        .llamar(json!({"op":"log","feature":"f","text":"algo"}))
        .await;
    assert!(log.is_error);
    assert_eq!(e.leer("f"), "hola sin cabecera\n");
}

#[tokio::test]
async fn un_documento_demasiado_grande_en_disco_se_rechaza_sin_leerlo_entero() {
    let e = entorno().await;
    std::fs::create_dir_all(e.ruta("f").parent().unwrap()).unwrap();
    let grande = "x".repeat(crate::odd::doc::MAX_DOC_BYTES + 10);
    std::fs::write(e.ruta("f"), &grande).unwrap();
    let out = e.llamar(json!({"op":"read","feature":"f"})).await;
    assert!(out.is_error);
    assert!(out.content.contains("bytes"), "{}", out.content);
    assert_eq!(e.leer("f"), grande);
}

#[tokio::test]
async fn dos_sesiones_con_features_distintas_no_se_pisan() {
    let e = entorno().await;
    let otra = e.store.create_session("proyecto").await.unwrap();
    let mut ctx_otra = e.ctx(true);
    ctx_otra.session = otra;

    e.crear_basico("alfa", &["Tarea de alfa"]).await;
    let out = e
        .llamar_con(
            ctx_otra,
            json!({"op":"create","feature":"beta","objective":"o","tasks":["Tarea de beta"]}),
        )
        .await;
    assert!(!out.is_error, "{}", out.content);

    assert_eq!(e.estado.active_feature(e.sesion).as_deref(), Some("alfa"));
    assert_eq!(e.estado.active_feature(otra).as_deref(), Some("beta"));
    assert_eq!(e.estado.active_feature(SessionId(999)), None);

    let md_alfa = e.leer("alfa");
    let md_beta = e.leer("beta");
    assert!(md_alfa.contains("Tarea de alfa") && !md_alfa.contains("Tarea de beta"));
    assert!(md_beta.contains("Tarea de beta") && !md_beta.contains("Tarea de alfa"));
}
