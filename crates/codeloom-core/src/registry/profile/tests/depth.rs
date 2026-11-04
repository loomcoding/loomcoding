//! Tests: depth.

use super::*;

#[test]
fn max_depth_zero_is_reported_as_not_applied_but_higher_values_are_accepted() {
    for valor in ["2", "3"] {
        let texto = format!(
            "---
name: frontend
description: Un agente de prueba
max_depth: {valor}
---

Cuerpo.
"
        );
        let (perfil, problemas) = parse_profile("frontend", &texto).unwrap();
        assert!(perfil.declared_not_applied.is_empty(), "{valor}");
        assert!(problemas.is_empty(), "{problemas:?}");
    }
    let con_cero = "---
name: frontend
description: Un agente de prueba
max_depth: 0
---

Cuerpo.
";
    let (perfil, problemas) = parse_profile("frontend", con_cero).unwrap();
    assert_eq!(perfil.declared_not_applied, vec!["max_depth".to_string()]);
    assert!(problemas.contains(&ProfileProblem::DeclaredNotApplied("max_depth".into())));
}

#[test]
fn max_depth_one_or_absent_is_not_reported() {
    let con_uno = "---
name: frontend
description: Un agente de prueba
max_depth: 1
---

Cuerpo.
";
    let sin_campo = "---
name: frontend
description: Un agente de prueba
---

Cuerpo.
";
    for texto in [con_uno, sin_campo] {
        let (perfil, problemas) = parse_profile("frontend", texto).unwrap();
        assert!(perfil.declared_not_applied.is_empty());
        assert!(problemas.is_empty(), "{problemas:?}");
    }
}

#[test]
fn declared_scope_is_reported_as_not_applied() {
    let texto = "---\nname: frontend\ndescription: Un agente de prueba\nscope:\n  tools: [view]\n---\n\nCuerpo.\n";
    let (perfil, problemas) = parse_profile("frontend", texto).unwrap();
    assert_eq!(perfil.declared_not_applied, vec!["scope".to_string()]);
    assert!(problemas.contains(&ProfileProblem::DeclaredNotApplied("scope".to_string())));
}
