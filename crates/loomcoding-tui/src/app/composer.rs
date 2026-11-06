//! Escritura en el composer, imágenes del borrador y barra lateral.

use super::*;

impl App {
    /// Texto pegado (bracketed paste): entra de una vez en el composer y
    /// nunca envía. Se ignora con un diálogo abierto o sin foco en el composer.
    pub fn paste(&mut self, text: &str) {
        if self.permission.is_some() || self.focus != Focus::Composer {
            return;
        }
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        self.composer.insert_paste(&text);
        self.history.reset();
    }

    pub fn attach_image_bytes(&mut self, bytes: &[u8], filename: &str) -> Result<(), String> {
        if self.draft_images.len() >= MAX_PROMPT_IMAGES {
            return Err(format!(
                "A prompt can contain at most {MAX_PROMPT_IMAGES} images"
            ));
        }
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("Image must be no larger than 2 MiB".into());
        }
        let (mime_type, signature_matches) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            ("image/png", true)
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            ("image/jpeg", true)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            ("image/gif", true)
        } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            ("image/webp", true)
        } else {
            ("", false)
        };
        if !signature_matches {
            return Err(
                "Unsupported image or invalid image signature (use PNG, JPEG, GIF or WebP)".into(),
            );
        }
        let filename = filename
            .chars()
            .map(|character| {
                if character.is_control() || matches!(character, '[' | ']') {
                    '_'
                } else {
                    character
                }
            })
            .collect::<String>();
        self.draft_images.push(ContentPart::Image {
            mime_type: mime_type.into(),
            data: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes),
            filename: filename.clone(),
        });
        self.history.reset();
        Ok(())
    }

    pub fn detach_last_image(&mut self) -> bool {
        if self.draft_images.pop().is_some() {
            self.history.reset();
            true
        } else {
            false
        }
    }

    pub fn take_draft_images(&mut self) -> Vec<ContentPart> {
        std::mem::take(&mut self.draft_images)
    }

    pub fn push_char(&mut self, c: char) {
        self.composer.insert_char(c);
        self.history.reset();
    }

    pub fn newline(&mut self) {
        self.composer.newline();
        self.history.reset();
    }

    pub fn backspace(&mut self) {
        self.composer.backspace();
        self.history.reset();
    }

    pub fn delete(&mut self) {
        self.composer.delete();
        self.history.reset();
    }

    pub fn move_left(&mut self) {
        self.composer.move_left();
    }

    pub fn move_right(&mut self) {
        self.composer.move_right();
    }

    pub fn move_home(&mut self) {
        self.composer.move_home();
    }

    pub fn move_end(&mut self) {
        self.composer.move_end();
    }

    /// ↑: cursor si hay varias líneas; si no, historial.
    pub fn up(&mut self) {
        if !self.history.is_browsing() && self.composer.lines() > 1 {
            self.composer.move_up();
            return;
        }
        let current = self.composer.text().to_string();
        if let Some(prev) = self.history.prev(&current) {
            let prev = prev.to_string();
            self.composer.set_text(&prev);
        }
    }

    /// ↓: historial si se está navegando; si no, cursor.
    pub fn down(&mut self) {
        if !self.history.is_browsing() {
            self.composer.move_down();
            return;
        }
        if let Some(next) = self.history.next() {
            let next = next.to_string();
            self.composer.set_text(&next);
        }
    }

    /// Nombres de los agentes que pueden lanzarse ahora: integrados, globales y aprobados.
    pub fn usable_agent_names(&self) -> Vec<String> {
        use loomcoding_core::registry::Approval;
        let Some(registry) = &self.agent_registry else {
            return Vec::new();
        };
        let snapshot = registry.snapshot();
        snapshot
            .entries()
            .into_iter()
            .filter(|entry| {
                matches!(
                    snapshot.approval(&entry.name),
                    Some(Approval::NotRequired | Approval::Approved)
                )
            })
            .map(|entry| entry.name.clone())
            .collect()
    }

    /// Si el texto del compositor es una orden de agente (`/agent …` o `@agente …`), la deja pedida en
    /// `run_agent_requested` y vacía el compositor; con un error de sintaxis deja el mensaje en
    /// `slash_feedback` y conserva el texto. Devuelve `true` si el texto era una orden de agente.
    pub fn try_agent_command(&mut self) -> bool {
        let text = self.composer.text().to_string();
        match crate::agent_command::parse_agent_command(&text, &self.usable_agent_names()) {
            None => false,
            Some(Err(message)) => {
                self.slash_feedback = Some(message);
                true
            }
            Some(Ok(invocation)) => {
                if self.composer.submit().is_some() {
                    self.history.push(text);
                }
                self.run_agent_requested = Some(invocation);
                true
            }
        }
    }

    pub fn submit(&mut self) -> Option<String> {
        let text = self.composer.submit()?;
        self.history.push(text.clone());
        Some(text)
    }

    /// Alterna la visibilidad lateral: el rail con el layout `ContextRail`, la tarjeta de agentes
    /// con `Cards` y el sidebar izquierdo con el resto. Cada uno conserva su propio estado.
    pub fn toggle_sidebar(&mut self) {
        use loomcoding_cloth::widgets::LayoutStyle;
        match self.theme.look.layout {
            LayoutStyle::ContextRail => self.rail_hidden = !self.rail_hidden,
            LayoutStyle::Cards => self.agents_card_folded = !self.agents_card_folded,
            _ => self.sidebar_expanded = !self.sidebar_expanded,
        }
    }

    /// Construye el menú izquierdo del layout `Workspace`: el logo y, después, bloques sin
    /// mayúsculas. Lo que vale cero no se pinta, y un bloque sin nada no existe.
    pub fn sidebar(&self) -> loomcoding_cloth::state::Sidebar {
        let mut sb = loomcoding_cloth::state::Sidebar::with_loom_mark("Loomcoding");

        // Primer bloque, sin título: estado, modelo · esfuerzo y ruta corta. Un estado de error
        // va con el estilo de error (línea 0).
        let glifo = if self.status == "working" {
            "◐"
        } else {
            "◌"
        };
        let errores: &[usize] = if self.status.starts_with("error:") {
            &[0]
        } else {
            &[]
        };
        let mut estado = vec![format!("{glifo} {}", self.status)];
        let esfuerzo = self
            .provider_settings
            .as_ref()
            .and_then(|ajustes| ajustes.effort.as_deref())
            .filter(|e| !e.is_empty());
        let modelo: Vec<&str> = [Some(self.model.as_str()), esfuerzo]
            .into_iter()
            .flatten()
            .filter(|parte| !parte.is_empty())
            .collect();
        if !modelo.is_empty() {
            estado.push(modelo.join(" · "));
        }
        if let Some(ruta) = ruta_corta(&self.cwd) {
            estado.push(ruta);
        }
        sb.push_section_with_errors("", estado, errores);

        // Actividad: una línea por cosa en marcha; lo que vale cero no se pinta.
        let mut actividad = Vec::new();
        let herramientas = self
            .tools
            .iter()
            .filter(|tool| tool.status() == loomcoding_cloth::state::ToolStatus::Running)
            .count();
        if herramientas > 0 {
            let palabra = if herramientas == 1 {
                "herramienta"
            } else {
                "herramientas"
            };
            actividad.push(format!("◌ {herramientas} {palabra}"));
        }
        let agentes = self.active_agents();
        if agentes > 0 {
            let palabra = if agentes == 1 { "agente" } else { "agentes" };
            actividad.push(format!("◐ {agentes} {palabra}"));
            // Trazabilidad A2A: el desglose por estado va entre paréntesis y, como la barra es
            // estrecha, partido en líneas de como mucho 20 columnas.
            actividad.extend(crate::rail::partir_por_palabras(
                &format!("({})", self.running_breakdown()),
                20,
            ));
        }
        let jobs = self
            .jobs
            .values()
            .filter(|job| Some(job.owner) == self.session_id)
            .filter(|job| {
                matches!(
                    job.status,
                    loomcoding_core::job::JobStatus::Running
                        | loomcoding_core::job::JobStatus::Stopping
                )
            })
            .count();
        let mut cola = Vec::new();
        if jobs > 0 {
            cola.push(format!("{jobs} {}", if jobs == 1 { "job" } else { "jobs" }));
        }
        if self.queued > 0 {
            cola.push(format!("{} en cola", self.queued));
        }
        if !cola.is_empty() {
            actividad.push(cola.join(" · "));
        }
        if !actividad.is_empty() {
            sb.push_section("Actividad", actividad);
        }

        // Sesión: los tokens abreviados; sin tokens no hay bloque.
        let mut tokens = Vec::new();
        if self.input_tokens > 0 {
            tokens.push(format!("↑{}", format_tokens(self.input_tokens)));
        }
        if self.output_tokens > 0 {
            tokens.push(format!("↓{}", format_tokens(self.output_tokens)));
        }
        if !tokens.is_empty() {
            sb.push_section("Sesión", vec![tokens.join(" ")]);
        }
        sb
    }
}

/// Ruta corta de `cwd`: «…/» y su último componente, con `\` o `/` como separador. Vacía si no
/// hay componente.
fn ruta_corta(cwd: &str) -> Option<String> {
    let ultimo = cwd.rsplit(['/', '\\']).find(|parte| !parte.is_empty())?;
    Some(format!("…/{ultimo}"))
}
