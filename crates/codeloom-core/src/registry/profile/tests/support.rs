//! Piezas comunes de los tests.

pub(super) fn perfil_completo() -> String {
    r#"---
name: frontend
description: Especialista en UI, componentes y accesibilidad
version: 1
persona:
  role: Ingeniero de frontend
  goal: Implementar y revisar UI del proyecto
  backstory: Prefiere componentes pequeños, testeables y accesibles
scope:
  mcp: [serena, figma]
  skills: [ponytail, accessibility]
  tools: [view, edit, glob, grep, bash]
model: claude-sonnet
effort: high
max_turns: 20
max_depth: 1
delegation:
  allow: true
  policy: both
transport: a2a-remote
a2a:
  endpoint: https://devops.internal/.well-known/agent-card.json
  card: devops
---

Eres el agente de frontend. Solo atiendes tareas de UI, componentes y accesibilidad.
Si la tarea es de otra especialidad, dilo y delega con `delegate`.
"#
    .to_string()
}

pub(super) fn perfil_minimo() -> String {
    "---\nname: frontend\ndescription: Un agente de prueba\n---\n\nEres el agente.\n".to_string()
}

pub(super) fn texto_con_nombre(nombre: &str) -> String {
    format!("---\nname: '{nombre}'\ndescription: Un agente de prueba\n---\n\nCuerpo.\n")
}

pub(super) fn perfil_remoto(frontmatter_extra: &str) -> String {
    format!("---\nname: lejano\ndescription: Agente remoto\n{frontmatter_extra}---\n")
}
