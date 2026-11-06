//! Temas, ajustes del proveedor, selectores y guardado.

use super::*;

impl App {
    /// Abre la ventana `/temas` en la raíz, partiendo de la config vigente.
    pub fn open_temas(&mut self) {
        self.temas_open = true;
        self.temas_level = TemasLevel::Root;
        self.temas_selected = 0;
        self.temas_matrix = self.matrix_rain.config().clone();
        self.temas_dirty = false;
        self.close_picker();
    }

    /// Cierra la ventana. Si hay cambios sin guardar, los descarta.
    pub fn close_temas(&mut self) {
        self.temas_open = false;
        self.temas_level = TemasLevel::Root;
        self.temas_selected = 0;
        self.temas_dirty = false;
    }

    /// Mueve la selección dentro del nivel actual, sin dar la vuelta.
    pub fn temas_move(&mut self, delta: isize) {
        let tope = self.temas_level.rows().saturating_sub(1);
        self.temas_selected = self.temas_selected.saturating_add_signed(delta).min(tope);
    }

    /// `Enter`: entra en el elemento seleccionado o aplica la acción del nivel.
    pub fn temas_enter(&mut self) {
        match self.temas_level {
            TemasLevel::Root => self.ui_root_enter(),
            TemasLevel::Effects => {
                self.temas_level = TemasLevel::Matrix;
                self.temas_selected = 0;
            }
            TemasLevel::Theme => {
                if let Some(&preset) =
                    loomcoding_cloth::widgets::ThemePreset::ALL.get(self.temas_selected)
                {
                    self.apply_theme(preset);
                    // El tema cambia la config base de la lluvia: se refresca
                    // la copia editable para no quedarse con la anterior.
                    self.temas_matrix = self.theme.matrix.clone();
                    self.temas_dirty = false;
                }
            }
            TemasLevel::Spinner => {
                if let Some(&choice) = SPINNER_CHOICES.get(self.temas_selected) {
                    self.spinner_choice = choice;
                    self.temas_dirty = true;
                    self.persist_spinner();
                    // El composer refleja la elección sin esperar a la próxima petición.
                    self.composer.set_activity_style(choice.style_for(0));
                    self.temas_dirty = false;
                }
            }
            TemasLevel::Matrix => {
                if self.temas_selected == MATRIX_FIELDS.len() - 1 {
                    self.temas_save();
                }
            }
        }
    }

    /// `Esc`: sube un nivel, o cierra la ventana si ya está en la raíz.
    pub fn temas_back(&mut self) {
        match self.temas_level {
            TemasLevel::Root => self.close_temas(),
            TemasLevel::Effects => {
                self.temas_level = TemasLevel::Root;
                self.temas_selected = UiRow::Effects.root_index();
            }
            TemasLevel::Matrix => {
                self.temas_level = TemasLevel::Effects;
                self.temas_selected = 0;
            }
            TemasLevel::Theme => {
                self.temas_level = TemasLevel::Root;
                self.temas_selected = UiRow::Theme.root_index();
            }
            TemasLevel::Spinner => {
                self.temas_level = TemasLevel::Root;
                self.temas_selected = UiRow::Spinner.root_index();
            }
        }
    }

    /// `←`/`→`: en la raíz cambia el valor de la fila sin entrar; en `Spinner` cambia el
    /// indicador elegido; en `Matrix` ajusta el campo seleccionado del formulario de Matrix rain.
    pub fn temas_adjust(&mut self, forward: bool) {
        if self.temas_level == TemasLevel::Root {
            self.ui_root_cycle(forward);
            return;
        }
        if self.temas_level == TemasLevel::Spinner {
            let count = SPINNER_CHOICES.len();
            let next = if forward {
                (self.temas_selected + 1) % count
            } else {
                (self.temas_selected + count - 1) % count
            };
            self.temas_selected = next;
            if let Some(&choice) = SPINNER_CHOICES.get(next) {
                self.spinner_choice = choice;
                self.composer.set_activity_style(choice.style_for(0));
                self.persist_spinner();
            }
            return;
        }
        if self.temas_level != TemasLevel::Matrix {
            return;
        }
        let mut config = self.temas_matrix.clone();
        match self.temas_selected {
            0 => {
                // Glifos: cicla por los presets.
                let presets = [
                    loomcoding_cloth::widgets::matrix_rain::GlyphSet::katakana(),
                    loomcoding_cloth::widgets::matrix_rain::GlyphSet::emoji(),
                    loomcoding_cloth::widgets::matrix_rain::GlyphSet::binary(),
                    loomcoding_cloth::widgets::matrix_rain::GlyphSet::ascii(),
                    loomcoding_cloth::widgets::matrix_rain::GlyphSet::mixed(),
                ];
                let current = presets
                    .iter()
                    .position(|p| *p == config.glyphs)
                    .unwrap_or(0);
                let next = if forward {
                    (current + 1) % presets.len()
                } else {
                    (current + presets.len() - 1) % presets.len()
                };
                config.glyphs = presets[next].clone();
            }
            1 => {
                let step = if forward { 0.05 } else { -0.05 };
                config.speed = (config.speed + step).clamp(0.05, 4.0);
            }
            2 => {
                let step: i16 = if forward { 1 } else { -1 };
                config.density = (i16::from(config.density) + step).clamp(0, 100) as u8;
            }
            3 => {
                let step: i32 = if forward { 1 } else { -1 };
                config.length = (i32::from(config.length) + step).clamp(1, 200) as u16;
            }
            4 => {
                // Color: cicla por los tokens del tema.
                let tokens = loomcoding_cloth::state::transcript::ALL_TOKENS;
                let current = tokens.iter().position(|t| *t == config.color).unwrap_or(0);
                let next = if forward {
                    (current + 1) % tokens.len()
                } else {
                    (current + tokens.len() - 1) % tokens.len()
                };
                config.color = tokens[next];
            }
            _ => return,
        }
        self.temas_matrix = config.clamped();
        self.temas_dirty = true;
        // Previsualización en vivo.
        self.matrix_rain.set_config(self.temas_matrix.clone());
    }

    /// Guarda la config de la lluvia en `ui.toml` y la deja aplicada.
    pub fn temas_save(&mut self) {
        self.matrix_rain.set_config(self.temas_matrix.clone());
        if let Some(path) = &self.prefs_path {
            let _ = crate::prefs::save_matrix(path, &self.temas_matrix);
        }
        self.temas_dirty = false;
    }

    /// Persiste el spinner elegido en `ui.toml`. Un fallo no impide el cambio:
    /// el spinner ya está aplicado en memoria y solo se avisa en `slash_feedback`.
    pub fn persist_spinner(&mut self) {
        let Some(path) = &self.prefs_path else {
            return;
        };
        if let Err(err) = crate::prefs::save_spinner(path, &self.spinner_choice.key()) {
            self.slash_feedback = Some(format!("Spinner not saved: {err}"));
        }
    }

    pub fn open_settings(&mut self) {
        if self.provider_settings.is_some() {
            self.settings_open = true;
            self.settings_feedback = None;
            self.settings_warned = false;
            self.close_picker();
            self.settings_baseline = self.provider_settings.clone();
            self.request_refresh_providers();
        }
    }

    /// `true` si los ajustes en pantalla difieren de la copia guardada.
    pub fn settings_dirty(&self) -> bool {
        !self.settings_api_key.is_empty()
            || match (&self.provider_settings, &self.settings_baseline) {
                (Some(current), Some(baseline)) => current != baseline,
                _ => false,
            }
    }

    /// `true` si el campo `index` (0 proveedor, 1 modelo, 2 URL, 3 API key,
    /// 4 esfuerzo) difiere de la copia guardada. El esfuerzo solo cuenta mientras el
    /// campo existe: un cambio que el modelo actual no puede mostrar no se marca.
    pub fn settings_field_changed(&self, index: usize) -> bool {
        match (&self.provider_settings, &self.settings_baseline) {
            (Some(current), Some(baseline)) => match index {
                0 => current.kind != baseline.kind,
                1 => current.model != baseline.model,
                2 => current.base_url != baseline.base_url,
                3 => !self.settings_api_key.is_empty(),
                SETTINGS_EFFORT_FIELD => {
                    self.settings_effort_visible() && current.effort != baseline.effort
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Número de campos con cambios sin guardar.
    pub fn settings_change_count(&self) -> usize {
        (0..=SETTINGS_EFFORT_FIELD)
            .filter(|&index| self.settings_field_changed(index))
            .count()
    }

    /// Niveles de esfuerzo que admite el modelo de los ajustes, según el catálogo (proveedor
    /// con la misma URL base y modelo con el mismo identificador) o, si el catálogo no publica niveles
    /// con nombre, los declarados en `[provider] efforts`. Vacío si no hay ni unos ni otros.
    pub fn effort_options(&self) -> Vec<String> {
        let Some(settings) = &self.provider_settings else {
            return Vec::new();
        };
        let from_catalog = self
            .model_catalog_provider()
            .and_then(|provider| {
                provider
                    .models
                    .iter()
                    .find(|model| model.id == settings.model)
            })
            .and_then(|model| model.reasoning.as_ref())
            .map(|reasoning| reasoning.efforts.clone())
            .unwrap_or_default();
        if from_catalog.is_empty() {
            self.declared_efforts.clone()
        } else {
            from_catalog
        }
    }

    /// `true` si `/settings` muestra el campo «Effort» (el modelo admite niveles).
    pub fn settings_effort_visible(&self) -> bool {
        !self.effort_options().is_empty()
    }

    /// Posición de «Save»: justo tras «Effort» si ese campo existe, o en su lugar si no.
    pub fn settings_save_field(&self) -> usize {
        if self.settings_effort_visible() {
            SETTINGS_EFFORT_FIELD + 1
        } else {
            SETTINGS_EFFORT_FIELD
        }
    }

    /// Avanza (o retrocede) el nivel de esfuerzo entre «default» y los niveles del modelo,
    /// dando la vuelta al final. Sin niveles disponibles no hace nada.
    pub fn cycle_effort(&mut self, forward: bool) {
        let options = self.effort_options();
        if options.is_empty() {
            return;
        }
        let Some(settings) = self.provider_settings.as_mut() else {
            return;
        };
        // La posición 0 es «default»; la i + 1 corresponde al nivel `options[i]`.
        let len = options.len() + 1;
        let current = settings
            .effort
            .as_ref()
            .and_then(|level| options.iter().position(|option| option == level))
            .map_or(0, |index| index + 1);
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        settings.effort = next.checked_sub(1).map(|index| options[index].clone());
    }

    /// Tras cambiar el modelo o la URL, descarta el nivel si el modelo nuevo no lo admite
    /// y lo deja anotado en `settings_feedback`.
    pub(super) fn reconcile_effort(&mut self) {
        let options = self.effort_options();
        let unsupported = self.provider_settings.as_ref().is_some_and(|settings| {
            settings
                .effort
                .as_ref()
                .is_some_and(|level| !options.contains(level))
        });
        if !unsupported {
            return;
        }
        if let Some(settings) = self.provider_settings.as_mut() {
            settings.effort = None;
        }
        self.settings_feedback = Some("Effort reset: not supported by this model".into());
    }

    /// Proveedor del catálogo cuya URL base coincide con la de los ajustes actuales.
    pub(super) fn model_catalog_provider(&self) -> Option<&CatalogProvider> {
        let current = self.provider_settings.as_ref()?;
        let wanted = normalize_base_url(&current.base_url);
        self.catalog
            .providers
            .iter()
            .find(|provider| normalize_base_url(&provider.base_url) == wanted)
    }

    /// Abre el selector del campo indicado. Sin catálogo, o sin proveedor del catálogo
    /// que coincida con la URL actual, no lo abre y deja el aviso en `settings_feedback`.
    pub fn open_picker(&mut self, picker: SettingsPicker) {
        if picker == SettingsPicker::Provider {
            self.request_refresh_providers();
        }
        if self.catalog.is_empty() && picker != SettingsPicker::Provider {
            self.settings_feedback = Some("Catalog not loaded yet; type it by hand".into());
            return;
        }
        if picker == SettingsPicker::Model && self.model_catalog_provider().is_none() {
            self.settings_feedback = Some("No catalog provider matches the current URL".into());
            return;
        }
        self.settings_picker = Some(picker);
        self.picker_query.clear();
        self.picker_selected = 0;
        self.settings_feedback = None;
    }

    /// Cierra el selector sin tocar los valores ni el diálogo.
    pub fn close_picker(&mut self) {
        self.settings_picker = None;
        self.picker_query.clear();
        self.picker_selected = 0;
    }

    /// Filas del selector abierto, filtradas por `picker_query` (sin distinguir mayúsculas,
    /// sobre el identificador y el nombre). Sin selector abierto, lista vacía.
    pub fn picker_items(&self) -> Vec<PickerItem> {
        let query = self.picker_query.trim().to_lowercase();
        let all: Vec<PickerItem> = match self.settings_picker {
            None => return Vec::new(),
            Some(SettingsPicker::Provider) => {
                let mut providers: Vec<PickerItem> = self
                    .catalog
                    .providers
                    .iter()
                    .map(|provider| PickerItem {
                        id: provider.id.clone(),
                        label: provider.name.clone(),
                        detail: provider.base_url.clone(),
                    })
                    .collect();
                providers.extend([
                    PickerItem {
                        id: "openai-responses".into(),
                        label: "OpenAI Responses".into(),
                        detail: "https://api.openai.com/v1".into(),
                    },
                    PickerItem {
                        id: "gemini-interactions".into(),
                        label: "Gemini Interactions".into(),
                        detail: "https://generativelanguage.googleapis.com/v1beta".into(),
                    },
                ]);
                providers
            }
            Some(SettingsPicker::Model) => self
                .model_catalog_provider()
                .map(|provider| {
                    provider
                        .models
                        .iter()
                        .map(|model| PickerItem {
                            id: model.id.clone(),
                            label: if model.name.is_empty() {
                                model.id.clone()
                            } else {
                                model.name.clone()
                            },
                            detail: model_detail(model),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        };
        all.into_iter()
            .filter(|item| {
                query.is_empty()
                    || item.id.to_lowercase().contains(&query)
                    || item.label.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Mueve la fila resaltada del selector, sin salirse de la lista filtrada.
    pub fn picker_move(&mut self, down: bool) {
        let last = self.picker_items().len().saturating_sub(1);
        self.picker_selected = if down {
            (self.picker_selected + 1).min(last)
        } else {
            self.picker_selected.saturating_sub(1)
        };
    }

    /// Aplica la fila resaltada y cierra el selector. Elegir un proveedor rellena tipo,
    /// URL base y variable de clave (si el catálogo la trae; si no, se conserva la
    /// actual). Elegir un modelo rellena el identificador. Lo rellenado cuenta como
    /// cambio sin guardar.
    pub fn picker_choose(&mut self) {
        let Some(picker) = self.settings_picker else {
            return;
        };
        let items = self.picker_items();
        // Un filtro sin coincidencias en la lista de modelos se acepta tal cual:
        // permite fijar a mano un modelo que el catálogo aún no conoce.
        if picker == SettingsPicker::Model && items.is_empty() && !self.picker_query.is_empty() {
            let typed = self.picker_query.trim().to_string();
            if let Some(settings) = self.provider_settings.as_mut() {
                settings.model = typed;
            }
            self.close_picker();
            self.settings_feedback = None;
            self.reconcile_effort();
            return;
        }
        if let (Some(item), Some(settings)) = (
            items.get(self.picker_selected),
            self.provider_settings.as_mut(),
        ) {
            match picker {
                SettingsPicker::Provider => {
                    self.settings_api_key.clear();
                    if let Some(provider) = self.catalog.provider(&item.id) {
                        settings.kind = provider.kind;
                        settings.base_url = provider.base_url.clone();
                        if !provider.api_key_env.is_empty() {
                            settings.api_key_env = provider.api_key_env.clone();
                        }
                    } else {
                        match item.id.as_str() {
                            "openai-responses" => {
                                settings.kind = ProviderKind::OpenAiResponses;
                                settings.base_url = "https://api.openai.com/v1".into();
                                settings.api_key_env = "OPENAI_API_KEY".into();
                            }
                            "gemini-interactions" => {
                                settings.kind = ProviderKind::Gemini;
                                settings.base_url =
                                    "https://generativelanguage.googleapis.com/v1beta".into();
                                settings.api_key_env = "GEMINI_API_KEY".into();
                            }
                            _ => {}
                        }
                    }
                }
                SettingsPicker::Model => settings.model = item.id.clone(),
            }
        }
        self.close_picker();
        self.settings_feedback = None;
        self.reconcile_effort();
        // Tras elegir proveedor se pasa directamente a sus modelos.
        if picker == SettingsPicker::Provider {
            self.settings_field = 1;
            if self.model_catalog_provider().is_some() {
                self.open_picker(SettingsPicker::Model);
            } else {
                self.settings_feedback =
                    Some("Enter this provider's model ID in the Model field".into());
            }
        }
    }

    /// Cierra `/settings`. Con cambios sin guardar avisa primero: devuelve `false`
    /// y deja el aviso en `settings_feedback`. Con el aviso ya dado descarta los
    /// cambios, restaura la copia guardada y devuelve `true`.
    pub fn close_settings(&mut self) -> bool {
        if self.settings_dirty() && !self.settings_warned {
            self.settings_warned = true;
            self.settings_feedback = Some(SETTINGS_DISCARD_WARNING.into());
            return false;
        }
        if self.settings_dirty() {
            self.provider_settings = self.settings_baseline.clone();
            self.settings_api_key.clear();
        }
        self.close_picker();
        self.settings_open = false;
        self.settings_warned = false;
        self.settings_feedback = None;
        true
    }

    /// Anula el aviso de «cambios sin guardar» tras cualquier acción distinta de `Esc`.
    pub fn settings_disarm(&mut self) {
        if self.settings_warned {
            self.settings_warned = false;
            if self.settings_feedback.as_deref() == Some(SETTINGS_DISCARD_WARNING) {
                self.settings_feedback = None;
            }
        }
    }

    pub fn edit_settings_value(&mut self) {
        let Some(settings) = &self.provider_settings else {
            return;
        };
        let value = match self.settings_field {
            1 => &settings.model,
            2 => &settings.base_url,
            3 => &self.settings_api_key,
            _ => return,
        };
        self.settings_editor.set_text(value);
        self.settings_editing = true;
    }

    pub fn commit_settings_value(&mut self) {
        self.settings_disarm();
        let value = self.settings_editor.text().to_string();
        if self.settings_field == 3 {
            self.settings_api_key = value;
        } else if let Some(settings) = self.provider_settings.as_mut() {
            match self.settings_field {
                1 => settings.model = value,
                2 => settings.base_url = value,
                _ => {}
            }
        }
        if matches!(self.settings_field, 1 | 2) {
            self.reconcile_effort();
        }
        self.settings_editing = false;
    }

    pub fn request_settings_save(&mut self) {
        if self.settings_saving {
            return;
        }
        if self.turn_in_flight {
            self.settings_feedback =
                Some("Wait for the active turn before applying settings".into());
            return;
        }
        let Some(settings) = self.provider_settings.as_mut() else {
            return;
        };
        settings.model = settings.model.trim().to_string();
        settings.base_url = settings.base_url.trim().to_string();
        settings.api_key_env = settings.api_key_env.trim().to_string();
        self.settings_api_key = self.settings_api_key.trim().to_string();
        if self.settings_api_key.len() > 4096 || self.settings_api_key.chars().any(char::is_control)
        {
            self.settings_feedback =
                Some("API key must be at most 4096 bytes and contain no control characters".into());
            return;
        }
        if settings.model.trim().is_empty()
            || settings.base_url.trim().is_empty()
            || settings.api_key_env.trim().is_empty()
        {
            self.settings_feedback = Some("Model, URL and API key variable are required".into());
            return;
        }
        self.settings_save_requested = true;
        self.settings_feedback = None;
    }

    pub fn settings_saved(&mut self, result: Result<bool, String>) {
        self.settings_saving = false;
        self.settings_feedback = Some(match result {
            Ok(configured) => {
                self.settings_api_key_configured = configured;
                self.settings_api_key.clear();
                if self.status.starts_with("API key missing") {
                    self.status = if configured {
                        "ready".into()
                    } else {
                        "API key missing · /settings".into()
                    };
                }
                if let Some(settings) = &self.provider_settings {
                    self.model = settings.model.clone();
                    self.settings_baseline = Some(settings.clone());
                }
                if configured {
                    "Settings saved and applied".into()
                } else {
                    "Settings saved; add an API key before sending prompts".into()
                }
            }
            Err(error) => error,
        });
        // Con la ventana cerrada (p. ej. `/model`), el resultado va al compositor.
        if !self.settings_open {
            self.slash_feedback = self.settings_feedback.clone();
        }
    }
}
