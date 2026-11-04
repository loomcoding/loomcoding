//! Registro de agentes: perfiles `<nombre>.md` de las fuentes, precedencia y resolución.
//!
//! El registro reúne agentes de varias fuentes (`source`), aplica la precedencia y entrega
//! instantáneas inmutables (`RegistrySnapshot`). Una instantánea no cambia nunca: una
//! recarga produce otra, y quien ya tiene una (p. ej. una tarea en curso) la conserva.
//!
//! La recarga en caliente, la vigilancia y la aprobación de los agentes de proyecto llegan
//! en tareas siguientes de `docs/plans/hechos/2026-10-22-codeloom-a2a-agent-registry.md`.

mod snapshot;

pub use snapshot::RegistrySnapshot;

#[cfg(test)]
mod tests;

pub mod profile;

pub mod router;

pub mod source;

pub mod trust;

pub mod watch;

use std::collections::{BTreeMap, HashSet};

use std::path::{Path, PathBuf};

use std::sync::{Arc, Mutex, RwLock};

use crate::card::AgentCard;

use crate::config::ConfirmDelegation;

use crate::event::{EventBus, LiveEvent};

pub use profile::{is_valid_agent_name, parse_profile, AgentProfile, ProfileProblem};

pub use source::{
    AgentEntry, AgentOrigin, AgentSource, BuiltInSource, Execution, FileSource, LoadProblem,
    LocalSpec, ProfileLevel, RemoteFetched, RemoteSpec, SourceLoad, AGENTE_POR_DEFECTO,
};

pub use trust::{content_hash, Approval, ApprovalError, TrustError, TrustStore};

pub use watch::WatcherHandle;

pub(crate) use source::catalog_root;

use trust::{clave_remota, Confianza};

/// Registro de agentes. Las fuentes se pasan de menor a mayor precedencia: la última gana.
pub struct AgentRegistry {
    sources: Vec<Box<dyn AgentSource>>,
    actual: RwLock<Arc<RegistrySnapshot>>,
    /// Serializa las recargas: cada una parte de la instantánea que dejó la anterior.
    recarga: Mutex<()>,
    confianza: Option<Confianza>,
    /// Tarjetas de remotos ya consultadas (o el motivo del fallo), por nombre de agente. La
    /// descarga es asíncrona y vive fuera del núcleo; aquí solo se guarda su resultado.
    remotas: Mutex<BTreeMap<String, RemotaConsultada>>,
}

/// Resultado de consultar la tarjeta de un remoto: la tarjeta y su huella, o el motivo del fallo.
pub type ResultadoRemoto = Result<(AgentCard, RemoteFetched), String>;

#[derive(Debug, Clone)]
struct RemotaConsultada {
    endpoint: String,
    resultado: ResultadoRemoto,
}

impl AgentRegistry {
    /// Crea el registro y carga las fuentes una vez.
    pub fn new(sources: Vec<Box<dyn AgentSource>>) -> Self {
        let inicial = Arc::new(cargar(&sources, None, None, &BTreeMap::new()));
        AgentRegistry {
            sources,
            actual: RwLock::new(inicial),
            recarga: Mutex::new(()),
            confianza: None,
            remotas: Mutex::new(BTreeMap::new()),
        }
    }

    /// Configura la aprobación de los agentes de proyecto y recalcula la instantánea vigente.
    ///
    /// `project_root` se canoniza. Con `trust_all_projects`, todo agente de proyecto se
    /// considera aprobado (opción `trust_project_agents` de `codeloom.toml`).
    pub fn with_trust(
        mut self,
        almacen: TrustStore,
        project_root: PathBuf,
        trust_all_projects: bool,
    ) -> Self {
        self.confianza = Some(Confianza::new(almacen, &project_root, trust_all_projects));
        // La instantánea inicial se recalcula: la aprobación depende de la confianza.
        let remotas = self.remotas_consultadas();
        let inicial = Arc::new(cargar(
            &self.sources,
            None,
            self.confianza.as_ref(),
            &remotas,
        ));
        *self.actual.get_mut().unwrap_or_else(|e| e.into_inner()) = inicial;
        self
    }

    /// Vuelve a cargar todas las fuentes, publica la nueva instantánea y la devuelve.
    pub fn reload(&self) -> Arc<RegistrySnapshot> {
        self.recargar().1
    }

    /// Recarga y publica `AgentRegistryChanged` en `bus`, solo si hubo diferencias.
    pub fn reload_and_publish(&self, bus: &EventBus) -> Arc<RegistrySnapshot> {
        let (antes, despues) = self.recargar();
        if let Some(evento) = diferencias(&antes, &despues) {
            bus.publish(evento);
        }
        despues
    }

    /// Instantánea vigente. Las que ya se entregaron no cambian.
    pub fn snapshot(&self) -> Arc<RegistrySnapshot> {
        let actual = self.actual.read().unwrap_or_else(|e| e.into_inner());
        Arc::clone(&actual)
    }

    /// Aprueba el agente de proyecto `name` tal como está cargado ahora: la huella es la de la
    /// instantánea vigente, es decir, la del contenido que la persona usuaria ha podido ver.
    /// Si el fichero cambia después, la aprobación deja de valer en la recarga siguiente.
    ///
    /// Un agente remoto se aprueba igual, pero con la huella de su punto de entrada y de su
    /// tarjeta ya descargada: si el remoto la cambia, la aprobación deja de valer.
    pub fn approve(&self, name: &str) -> Result<(), ApprovalError> {
        let (confianza, huella, catalogo) = self.agente_aprobable(name, true)?;
        match catalogo {
            Some(raiz) => Ok(confianza.aprobar_catalogo(&raiz, name, &huella)?),
            None => Ok(confianza.aprobar(name, &huella)?),
        }
    }

    /// Revoca la aprobación del agente de proyecto `name`. Si no estaba aprobado, no hace nada.
    pub fn revoke(&self, name: &str) -> Result<(), ApprovalError> {
        let (confianza, _, catalogo) = self.agente_aprobable(name, false)?;
        match catalogo {
            Some(raiz) => Ok(confianza.revocar_catalogo(&raiz, name)?),
            None => Ok(confianza.revocar(name)?),
        }
    }

    /// Comprueba que `name` existe y requiere aprobación (de proyecto, o remoto); devuelve la
    /// confianza y la huella que se aprueba. Para revocar, `exigir_tarjeta` es falso: no hace
    /// falta tarjeta descargada y la huella devuelta no se usa.
    fn agente_aprobable(
        &self,
        name: &str,
        exigir_tarjeta: bool,
    ) -> Result<(&Confianza, String, Option<PathBuf>), ApprovalError> {
        let instantanea = self.snapshot();
        let entrada = instantanea
            .get(name)
            .ok_or_else(|| ApprovalError::Desconocido(name.to_string()))?;
        let mut catalogo = None;
        let huella = match (&entrada.execution, &entrada.origin, &entrada.content_hash) {
            (Execution::Remote(spec), _, _) => match &spec.fetched {
                Some(descargada) => clave_remota(&spec.endpoint, &descargada.card_hash),
                None if exigir_tarjeta => {
                    return Err(ApprovalError::TarjetaPendiente(name.to_string()))
                }
                None => String::new(),
            },
            (
                _,
                AgentOrigin::File {
                    level: ProfileLevel::Project,
                    ..
                },
                Some(huella),
            ) => huella.clone(),
            (
                _,
                AgentOrigin::File {
                    level: ProfileLevel::Catalog,
                    path,
                },
                Some(huella),
            ) => {
                catalogo = catalog_root(path);
                huella.clone()
            }
            _ => return Err(ApprovalError::NoRequerida(name.to_string())),
        };
        let confianza = self.confianza.as_ref().ok_or(ApprovalError::SinAlmacen)?;
        Ok((confianza, huella, catalogo))
    }

    /// Copia de las tarjetas remotas ya consultadas.
    fn remotas_consultadas(&self) -> BTreeMap<String, RemotaConsultada> {
        self.remotas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Anota el resultado de consultar la tarjeta del remoto `name` (cuyo punto de entrada es
    /// `endpoint`) y recalcula la instantánea. Si el perfil cambia de `endpoint`, lo anotado deja
    /// de valer. Publica `AgentRegistryChanged` en `bus` si algo cambió.
    pub fn set_remote_card(
        &self,
        bus: &EventBus,
        name: &str,
        endpoint: &str,
        resultado: ResultadoRemoto,
    ) -> Arc<RegistrySnapshot> {
        self.remotas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                name.to_string(),
                RemotaConsultada {
                    endpoint: endpoint.to_string(),
                    resultado,
                },
            );
        let (antes, despues) = self.recargar();
        if let Some(evento) = diferencias(&antes, &despues) {
            bus.publish(evento);
        }
        despues
    }

    /// Recarga bajo el cerrojo de recargas. Devuelve (anterior, nueva).
    fn recargar(&self) -> (Arc<RegistrySnapshot>, Arc<RegistrySnapshot>) {
        let _turno = self.recarga.lock().unwrap_or_else(|e| e.into_inner());
        let antes = self.snapshot();
        let remotas = self.remotas_consultadas();
        let despues = Arc::new(cargar(
            &self.sources,
            Some(&antes),
            self.confianza.as_ref(),
            &remotas,
        ));
        let mut actual = self.actual.write().unwrap_or_else(|e| e.into_inner());
        *actual = Arc::clone(&despues);
        (antes, despues)
    }
}

/// Carga las fuentes en orden y aplica la precedencia: la última que aporte un nombre gana,
/// y la entrada vencida queda anotada como `Shadowed`.
///
/// Con `previa`, aplica la regla de «última versión válida» (ver `conservar_ultimas_validas`).
fn cargar(
    sources: &[Box<dyn AgentSource>],
    previa: Option<&RegistrySnapshot>,
    confianza: Option<&Confianza>,
    remotas: &BTreeMap<String, RemotaConsultada>,
) -> RegistrySnapshot {
    let mut entries: BTreeMap<String, AgentEntry> = BTreeMap::new();
    let mut problems = Vec::new();
    for source in sources {
        let mut carga = source.load();
        if let Some(previa) = previa {
            conservar_ultimas_validas(&mut carga, previa);
        }
        problems.extend(carga.problems);
        for entry in carga.entries {
            if let Some(vencida) = entries.remove(&entry.name) {
                problems.push(LoadProblem::Shadowed {
                    name: entry.name.clone(),
                    by: entry.origin.clone(),
                    loser: vencida.origin,
                });
            }
            entries.insert(entry.name.clone(), entry);
        }
    }
    aplicar_remotas(&mut entries, remotas, &mut problems);
    RegistrySnapshot {
        entries,
        problems,
        confianza: confianza.cloned(),
    }
}

/// Funde en las entradas remotas lo consultado de su tarjeta. Siempre parte de «sin descargar»:
/// una entrada reinyectada de la instantánea anterior no arrastra una tarjeta que ya no consta.
fn aplicar_remotas(
    entries: &mut BTreeMap<String, AgentEntry>,
    remotas: &BTreeMap<String, RemotaConsultada>,
    problems: &mut Vec<LoadProblem>,
) {
    for entrada in entries.values_mut() {
        let Execution::Remote(spec) = &mut entrada.execution else {
            continue;
        };
        spec.fetched = None;
        let Some(consultada) = remotas.get(&entrada.name) else {
            continue;
        };
        if consultada.endpoint != spec.endpoint {
            continue;
        }
        match &consultada.resultado {
            Ok((tarjeta, descargada)) => {
                entrada.card = tarjeta.clone();
                spec.fetched = Some(descargada.clone());
            }
            Err(error) => problems.push(LoadProblem::Remote {
                name: entrada.name.clone(),
                error: error.clone(),
            }),
        }
    }
}

/// Regla de «última versión válida». Si un fichero que cargaba antes falla ahora (o su
/// directorio es ilegible), su entrada anterior se reinyecta en esta carga y el problema se
/// conserva. Un fichero borrado no produce problema: su entrada desaparece.
fn conservar_ultimas_validas(carga: &mut SourceLoad, previa: &RegistrySnapshot) {
    let fallidas: HashSet<&Path> = carga
        .problems
        .iter()
        .filter_map(|problema| match problema {
            LoadProblem::Profile { path, .. } | LoadProblem::Unreadable { path, .. } => {
                Some(path.as_path())
            }
            LoadProblem::Shadowed { .. } | LoadProblem::Remote { .. } => None,
        })
        .collect();

    let mut retenidas = Vec::new();
    for entrada in previa.entries.values() {
        let AgentOrigin::File { path, .. } = &entrada.origin else {
            continue;
        };
        let afectada = fallidas.contains(path.as_path())
            || path.parent().is_some_and(|dir| fallidas.contains(dir));
        let presente = carga
            .entries
            .iter()
            .any(|nueva| matches!(&nueva.origin, AgentOrigin::File { path: p, .. } if p == path));
        if afectada && !presente {
            retenidas.push(entrada.clone());
        }
    }
    carga.entries.extend(retenidas);
}

/// Diferencias entre dos instantáneas. `None` si no cambió nada visible: ni entradas ni
/// problemas.
fn diferencias(antes: &RegistrySnapshot, despues: &RegistrySnapshot) -> Option<LiveEvent> {
    let added: Vec<String> = despues
        .entries
        .keys()
        .filter(|nombre| !antes.entries.contains_key(*nombre))
        .cloned()
        .collect();
    let removed: Vec<String> = antes
        .entries
        .keys()
        .filter(|nombre| !despues.entries.contains_key(*nombre))
        .cloned()
        .collect();
    let changed: Vec<String> = despues
        .entries
        .iter()
        .filter(|(nombre, nueva)| {
            antes
                .entries
                .get(*nombre)
                .is_some_and(|previa| previa != *nueva)
        })
        .map(|(nombre, _)| nombre.clone())
        .collect();

    let cambio_de_problemas = antes.problems != despues.problems;
    if added.is_empty() && removed.is_empty() && changed.is_empty() && !cambio_de_problemas {
        return None;
    }
    Some(LiveEvent::AgentRegistryChanged {
        added,
        removed,
        changed,
        problems: despues.problems.len(),
    })
}
