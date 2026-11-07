//! Medición de ODD: contadores sin datos personales.
//!
//! Un contador no guarda rutas, texto de tareas ni identificadores de sesión; solo cuenta. Cada
//! incremento emite una línea al log del daemon (destino `odd`) con el nombre fijo del evento y el
//! total. No se publican en el bus: se leen desde el log y desde [`OddState::estadisticas`].

use std::sync::atomic::{AtomicU64, Ordering};

/// Lo que se mide. Cada variante sube exactamente un contador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Medida {
    DocumentoCreado,
    TareaMarcada,
    TareaBloqueada,
    CierreBloqueado,
    CierreSinProgreso,
    CierreDecision,
    CierreIlegible,
    RechazoEvidencia,
}

impl Medida {
    /// Nombre del evento en el log. Texto fijo: nada que venga de la persona usuaria.
    pub(crate) fn nombre(self) -> &'static str {
        match self {
            Medida::DocumentoCreado => "documento_creado",
            Medida::TareaMarcada => "tarea_marcada",
            Medida::TareaBloqueada => "tarea_bloqueada",
            Medida::CierreBloqueado => "cierre_bloqueado",
            Medida::CierreSinProgreso => "cierre_sin_progreso",
            Medida::CierreDecision => "cierre_por_decision",
            Medida::CierreIlegible => "cierre_por_documento_ilegible",
            Medida::RechazoEvidencia => "rechazo_de_evidencia",
        }
    }
}

/// Fotografía de los contadores de ODD desde que arrancó el proceso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OddStats {
    pub documentos_creados: u64,
    pub tareas_marcadas: u64,
    pub tareas_bloqueadas: u64,
    /// Cada reinyección del cierre, es decir, cada vez que el turno volvió al modelo.
    pub cierres_bloqueados: u64,
    pub cierres_sin_progreso: u64,
    /// Cierres por «Necesita tu decisión»: todas las tareas no hechas están bloqueadas.
    pub cierres_por_decision: u64,
    pub cierres_por_documento_ilegible: u64,
    pub rechazos_de_evidencia: u64,
}

/// Contadores vivos. Atómicos: la herramienta y la política los suben sin cerrojo.
#[derive(Default)]
pub(crate) struct Contadores {
    documentos_creados: AtomicU64,
    tareas_marcadas: AtomicU64,
    tareas_bloqueadas: AtomicU64,
    cierres_bloqueados: AtomicU64,
    cierres_sin_progreso: AtomicU64,
    cierres_por_decision: AtomicU64,
    cierres_por_documento_ilegible: AtomicU64,
    rechazos_de_evidencia: AtomicU64,
}

impl Contadores {
    fn de(&self, medida: Medida) -> &AtomicU64 {
        match medida {
            Medida::DocumentoCreado => &self.documentos_creados,
            Medida::TareaMarcada => &self.tareas_marcadas,
            Medida::TareaBloqueada => &self.tareas_bloqueadas,
            Medida::CierreBloqueado => &self.cierres_bloqueados,
            Medida::CierreSinProgreso => &self.cierres_sin_progreso,
            Medida::CierreDecision => &self.cierres_por_decision,
            Medida::CierreIlegible => &self.cierres_por_documento_ilegible,
            Medida::RechazoEvidencia => &self.rechazos_de_evidencia,
        }
    }

    /// Suma uno y devuelve el total nuevo.
    pub(crate) fn sumar(&self, medida: Medida) -> u64 {
        self.de(medida).fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn fotografia(&self) -> OddStats {
        let leer = |contador: &AtomicU64| contador.load(Ordering::Relaxed);
        OddStats {
            documentos_creados: leer(&self.documentos_creados),
            tareas_marcadas: leer(&self.tareas_marcadas),
            tareas_bloqueadas: leer(&self.tareas_bloqueadas),
            cierres_bloqueados: leer(&self.cierres_bloqueados),
            cierres_sin_progreso: leer(&self.cierres_sin_progreso),
            cierres_por_decision: leer(&self.cierres_por_decision),
            cierres_por_documento_ilegible: leer(&self.cierres_por_documento_ilegible),
            rechazos_de_evidencia: leer(&self.rechazos_de_evidencia),
        }
    }
}
