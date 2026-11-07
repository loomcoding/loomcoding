//! Proyección del documento de feature a la lista de tareas de la interfaz (`LiveEvent::TodosChanged`)
//! y al avance que muestra la cabecera.
//!
//! El documento es la fuente de verdad: la lista es una proyección suya, nunca otra fuente. Se
//! republica tras cada operación de `odd_track` que modifica el documento.
//!
//! NOTA: `todo_write` del modelo y esta proyección comparten el evento `TodosChanged`. Gana el último
//! en escribir en pantalla, y el documento es la verdad: si el modelo usa `todo_write` mientras hay un
//! documento de ODD, la lista de pantalla puede quedar desfasada. No se sincroniza hacia atrás.

use super::doc::{limit_bytes, FeatureDoc, Task, TaskStatus};
use super::observer::resumen_de_flujo;
use super::state::OddState;
use loomcoding_core::event::LiveEvent;
use loomcoding_core::todo::{TodoItem, TodoStatus, MAX_TODOS};
use loomcoding_core::tools::ToolCtx;

/// Bytes máximos del contenido de una entrada de la lista, sufijo de bloqueo incluido.
pub const MAX_CONTENIDO_BYTES: usize = 200;
/// Bytes máximos del motivo de bloqueo que se muestra en la lista.
const MAX_MOTIVO_EN_LISTA_BYTES: usize = 80;
/// Motivo mostrado si una tarea bloqueada no lo tiene.
const SIN_MOTIVO: &str = "sin motivo";

/// Lista de tareas que muestra la interfaz para el documento. Una entrada por tarea, en orden. Si
/// hay más de [`MAX_TODOS`], se muestran las primeras `MAX_TODOS - 1` y un resumen final pendiente.
pub fn project(doc: &FeatureDoc) -> Vec<TodoItem> {
    let total = doc.tasks.len();
    if total <= MAX_TODOS {
        return doc.tasks.iter().map(entrada).collect();
    }
    let visibles = MAX_TODOS - 1;
    let mut items: Vec<TodoItem> = doc.tasks[..visibles].iter().map(entrada).collect();
    items.push(TodoItem {
        content: format!("(+{} más)", total - visibles),
        status: TodoStatus::Pending,
    });
    items
}

/// Avance del documento: (tareas hechas, total de tareas).
pub fn progress(doc: &FeatureDoc) -> (usize, usize) {
    let hechas = doc
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Done)
        .count();
    (hechas, doc.tasks.len())
}

/// Publica la proyección de un documento recién guardado: la lista de tareas y, con ODD encendido,
/// el avance en la cabecera. El flujo solo se publica si cambia su etiqueta completa.
pub(crate) fn publicar(estado: &OddState, ctx: &ToolCtx, doc: &FeatureDoc) {
    let avance = progress(doc);
    estado.recordar_progreso(ctx.session, avance);
    ctx.bus.publish(LiveEvent::TodosChanged {
        session: ctx.session,
        items: project(doc),
    });
    if !estado.is_enabled() {
        return;
    }
    let previo = estado.publicado(ctx.session);
    let fase = previo.as_ref().map_or("planning", |p| p.fase);
    let resumen = resumen_de_flujo(fase, Some(avance));
    if previo.is_some_and(|p| p.etiqueta == resumen.label) {
        return;
    }
    estado.recordar_publicado(ctx.session, fase, &resumen.label);
    ctx.bus.publish(LiveEvent::FlowChanged {
        summary: resumen,
        actor: "loom".into(),
        detail: String::new(),
    });
}

fn entrada(tarea: &Task) -> TodoItem {
    let status = match tarea.status {
        TaskStatus::Done => TodoStatus::Completed,
        TaskStatus::Pending | TaskStatus::Blocked => TodoStatus::Pending,
    };
    TodoItem {
        content: contenido(tarea),
        status,
    }
}

/// Texto de la entrada. La tarea bloqueada termina en ` [bloqueada: motivo]`; el sufijo nunca se
/// recorta, así que el recorte del texto de la tarea deja sitio para él.
fn contenido(tarea: &Task) -> String {
    let base = format!("{}: {}", tarea.id, tarea.text);
    if tarea.status != TaskStatus::Blocked {
        return limit_bytes(&base, MAX_CONTENIDO_BYTES)
            .trim_end()
            .to_string();
    }
    let motivo = tarea.reason.as_deref().unwrap_or(SIN_MOTIVO);
    let sufijo = format!(
        " [bloqueada: {}]",
        limit_bytes(motivo, MAX_MOTIVO_EN_LISTA_BYTES)
    );
    let cabida = MAX_CONTENIDO_BYTES - sufijo.len();
    format!("{}{sufijo}", limit_bytes(&base, cabida).trim_end())
}
