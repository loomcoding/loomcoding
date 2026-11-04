//! Tests: mtls.

use super::support::*;
use super::*;

#[test]
fn mtls_reads_the_identity_file_and_never_shows_the_key() {
    let plan = plan_mtls(IDENTIDAD).unwrap();

    let identidad = plan.client_identity().expect("el plan exige una identidad");
    assert!(identidad.pem().starts_with(b"#") || identidad.pem().contains(&b'-'));
    let texto = format!("{plan:?} {identidad:?}");
    assert!(
        !texto.contains("PRIVATE") && !texto.contains("BEGIN"),
        "{texto}"
    );
    assert!(!plan.is_open());
}

#[test]
fn an_unusable_identity_file_is_reported_with_its_reason() {
    let dir = tempfile::tempdir().unwrap();
    let no_pem = dir.path().join("no.pem");
    std::fs::write(&no_pem, "esto no es un PEM").unwrap();
    let grande = dir.path().join("grande.pem");
    std::fs::write(&grande, vec![b'a'; crate::remote::MAX_PEM_BYTES + 1]).unwrap();
    let solo_certificado = dir.path().join("cert.pem");
    let completo = std::fs::read_to_string(IDENTIDAD).unwrap();
    let cert: String = completo
        .split("-----END CERTIFICATE-----")
        .next()
        .unwrap()
        .to_string()
        + "-----END CERTIFICATE-----\n";
    std::fs::write(&solo_certificado, cert).unwrap();

    for (ruta, esperado) in [
        ("/no/existe.pem".to_string(), "no se pudo abrir"),
        (no_pem.display().to_string(), "no es un PEM"),
        (grande.display().to_string(), "pasa de"),
        (solo_certificado.display().to_string(), "no es un PEM"),
    ] {
        match plan_mtls(&ruta) {
            Err(AuthProblem::BadIdentity { reason, env, .. }) => {
                assert!(reason.contains(esperado), "{reason} no contiene {esperado}");
                assert_eq!(env, "V");
            }
            otro => panic!("{ruta}: {otro:?}"),
        }
    }
}

#[test]
fn mtls_without_a_declared_credential_is_a_missing_credential() {
    let error = AuthPlan::choose(&tarjeta_mtls(), &credenciales(&[]), &entorno(&[])).unwrap_err();

    assert!(matches!(error, AuthProblem::MissingCredential { .. }));
}
