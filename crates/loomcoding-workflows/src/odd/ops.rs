//! Operaciones de `odd_track` sobre el documento en memoria. Puras: sin disco ni almacén.
//!
//! Todo texto que viene del modelo pasa por [`comprobar_texto`]: no vacío, una sola línea, sin los
//! separadores que reserva el formato y dentro de su tope. Nada se recorta en silencio.

use super::doc::{
    FeatureDoc, Task, TaskStatus, MAX_DOC_BYTES, MAX_EVIDENCE_BYTES, MAX_LOG_LINE_BYTES, MAX_TASKS,
    MAX_TASK_TEXT_BYTES,
};
use super::evidence::{formato, MAX_IDENTIFICADOR_LLAMADA};
use super::render::{BLOCK_MARK, EVIDENCE_MARK};
use loomcoding_core::tools::limit_bytes;

/// Bytes máximos del objetivo de una feature.
pub const MAX_OBJETIVO_BYTES: usize = 1_000;

/// Comprueba un texto que viene del modelo: no vacío, una sola línea, sin separadores del formato
/// y dentro de `max` bytes. Se mide ya recortado de espacios, como se guardará.
pub fn comprobar_texto(campo: &str, texto: &str, max: usize) -> Result<(), String> {
    let texto = texto.trim();
    if texto.is_empty() {
        return Err(format!("`{campo}` no puede estar vacío"));
    }
    if texto.contains('\n') || texto.contains('\r') {
        return Err(format!(
            "`{campo}` no admite un salto de línea; escríbalo en una sola línea"
        ));
    }
    if texto.contains(EVIDENCE_MARK) || texto.contains(BLOCK_MARK) {
        return Err(format!(
            "`{campo}` contiene un separador que reserva el formato (`— evidencia:` o \
             `— bloqueada:`); reescríbalo sin él"
        ));
    }
    if texto.len() > max {
        return Err(format!(
            "`{campo}` ocupa {} bytes; el máximo es {max}",
            texto.len()
        ));
    }
    Ok(())
}

/// Valida la evidencia (texto y llamada) y devuelve la forma guardada en la tarea.
pub fn formatear_evidencia(texto: &str, call_id: &str) -> Result<String, String> {
    comprobar_texto("evidence.text", texto, MAX_EVIDENCE_BYTES)?;
    let call_id = call_id.trim();
    let identificador_valido = !call_id.is_empty()
        && call_id.len() <= MAX_IDENTIFICADOR_LLAMADA
        && call_id.chars().all(|c| c.is_ascii_graphic() && c != ']');
    if !identificador_valido {
        return Err(format!(
            "`evidence.call_id` no es un identificador válido: use el id de una llamada, sin \
             espacios ni `]` y de hasta {MAX_IDENTIFICADOR_LLAMADA} bytes"
        ));
    }
    formato(texto.trim(), call_id)
}

/// Crea un documento nuevo con tareas `T1`, `T2`, ... Rechaza cualquier texto fuera de cotas.
pub fn crear(feature: &str, objetivo: &str, tareas: &[String]) -> Result<FeatureDoc, String> {
    comprobar_texto("objective", objetivo, MAX_OBJETIVO_BYTES)?;
    if tareas.is_empty() {
        return Err("`tasks` debe tener al menos una tarea".to_string());
    }
    if tareas.len() > MAX_TASKS {
        return Err(format!(
            "hay {} tareas; el máximo es {MAX_TASKS}. Agrupe el trabajo en tareas más grandes",
            tareas.len()
        ));
    }
    let mut lista = Vec::with_capacity(tareas.len());
    for (i, texto) in tareas.iter().enumerate() {
        comprobar_texto(&format!("tasks[{i}]"), texto, MAX_TASK_TEXT_BYTES)?;
        lista.push(Task::new(&format!("T{}", i + 1), texto));
    }
    let primera = lista[0].id.clone();
    let mut doc = FeatureDoc {
        feature: feature.to_string(),
        objective: objetivo.trim().to_string(),
        tasks: lista,
        next_step: format!("Empezar por {primera}."),
        ..FeatureDoc::default()
    };
    doc.push_log(&format!("Documento creado con {} tareas.", tareas.len()));
    comprobar_ida_y_vuelta(&doc)?;
    Ok(doc)
}

/// Marca una tarea como hecha con la evidencia ya formateada por [`formatear_evidencia`].
pub fn marcar_hecha(doc: &mut FeatureDoc, id: &str, evidencia: &str) -> Result<String, String> {
    let i = indice(doc, id)?;
    if doc.tasks[i].status == TaskStatus::Done {
        return Err(format!("la tarea {id} ya está hecha"));
    }
    let tarea = &mut doc.tasks[i];
    tarea.status = TaskStatus::Done;
    tarea.evidence = Some(evidencia.to_string());
    tarea.reason = None;
    anotar(doc, format!("{id} marcada como hecha."))?;
    Ok(format!("{id} marcada como hecha: evidencia {evidencia}."))
}

/// Devuelve una tarea `Done` o `Blocked` a pendiente, con su motivo en el registro.
pub fn reabrir(doc: &mut FeatureDoc, id: &str, motivo: &str) -> Result<String, String> {
    comprobar_texto("reason", motivo, MAX_TASK_TEXT_BYTES)?;
    let i = indice(doc, id)?;
    if doc.tasks[i].status == TaskStatus::Pending {
        return Err(format!("la tarea {id} ya está pendiente"));
    }
    let tarea = &mut doc.tasks[i];
    tarea.status = TaskStatus::Pending;
    tarea.evidence = None;
    tarea.reason = None;
    anotar(doc, format!("{id} reabierta: {}", motivo.trim()))?;
    Ok(format!("{id} reabierta: {}.", motivo.trim()))
}

/// Bloquea una tarea pendiente o bloqueada con un motivo. Una tarea hecha hay que reabrirla antes.
pub fn bloquear(doc: &mut FeatureDoc, id: &str, motivo: &str) -> Result<String, String> {
    comprobar_texto("reason", motivo, MAX_TASK_TEXT_BYTES)?;
    let i = indice(doc, id)?;
    if doc.tasks[i].status == TaskStatus::Done {
        return Err(format!(
            "la tarea {id} ya está hecha; reábrala antes de bloquearla"
        ));
    }
    let tarea = &mut doc.tasks[i];
    tarea.status = TaskStatus::Blocked;
    tarea.evidence = None;
    tarea.reason = Some(motivo.trim().to_string());
    anotar(doc, format!("{id} bloqueada: {}", motivo.trim()))?;
    Ok(format!("{id} bloqueada: {}.", motivo.trim()))
}

/// Añade una línea al registro.
pub fn registrar(doc: &mut FeatureDoc, texto: &str) -> Result<String, String> {
    comprobar_texto("text", texto, MAX_LOG_LINE_BYTES)?;
    doc.push_log(texto.trim());
    Ok("Registrado.".to_string())
}

/// Garantiza que el documento se puede releer tal cual después de escribirse. Si no, no se escribe.
pub fn comprobar_ida_y_vuelta(doc: &FeatureDoc) -> Result<(), String> {
    let markdown = doc.to_markdown();
    if markdown.len() > MAX_DOC_BYTES {
        return Err(format!(
            "el documento quedaría con {} bytes; el máximo es {MAX_DOC_BYTES}. No se guarda nada",
            markdown.len()
        ));
    }
    match FeatureDoc::parse(&markdown) {
        Ok(releido) if releido == *doc => Ok(()),
        Ok(_) => Err(
            "el cambio no se podría releer tal cual (¿un texto que empieza por `## `?); \
             no se guarda nada"
                .to_string(),
        ),
        Err(error) => Err(format!(
            "el cambio dejaría el documento roto ({error}); no se guarda nada"
        )),
    }
}

/// Posición de la tarea `id`, o un error que enumera las que existen.
fn indice(doc: &FeatureDoc, id: &str) -> Result<usize, String> {
    doc.tasks.iter().position(|t| t.id == id).ok_or_else(|| {
        let ids: Vec<&str> = doc.tasks.iter().map(|t| t.id.as_str()).collect();
        format!(
            "no existe la tarea `{}`; las tareas son: {}",
            limit_bytes(id, 32),
            ids.join(", ")
        )
    })
}

/// Añade al registro una línea de tarea. Si no cabe, se rechaza en lugar de recortarla.
fn anotar(doc: &mut FeatureDoc, linea: String) -> Result<(), String> {
    if linea.len() > MAX_LOG_LINE_BYTES {
        return Err(format!(
            "la línea del registro ocuparía {} bytes; el máximo es {MAX_LOG_LINE_BYTES}. \
             Acorte el motivo",
            linea.len()
        ));
    }
    doc.push_log(&linea);
    Ok(())
}
