//! Tipos del panel de ajustes, selectores, temas, spinners y catálogos de muestra.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SessionChoice {
    Resume(SessionId),
    New,
}

/// Aviso que se muestra al cerrar `/settings` con cambios sin guardar.
pub(super) const SETTINGS_DISCARD_WARNING: &str = "Unsaved changes · Esc again to discard";

/// Catálogo de muestra (formato de models.dev, recortado) para las pruebas del selector.
#[cfg(test)]
pub(crate) const PICKER_SAMPLE_JSON: &str = r#"{
    "openrouter": {"id": "openrouter", "name": "OpenRouter", "npm": "@openrouter/ai-sdk-provider",
        "api": "https://openrouter.ai/api/v1", "env": ["OPENROUTER_API_KEY"],
        "models": {"qwen": {"id": "qwen", "name": "Qwen Coder", "tool_call": true,
            "release_date": "2025-06-01", "limit": {"context": 262000, "output": 8000},
            "cost": {"input": 0.3, "output": 1.2}}}},
    "anthropic": {"id": "anthropic", "name": "Anthropic", "npm": "@ai-sdk/anthropic",
        "env": ["ANTHROPIC_API_KEY"],
        "models": {
            "claude-opus": {"id": "claude-opus", "name": "Claude Opus", "tool_call": true,
                "release_date": "2025-05-01", "limit": {"context": 200000, "output": 32000}},
            "claude-sonnet": {"id": "claude-sonnet", "name": "Claude Sonnet", "tool_call": true,
                "release_date": "2025-09-01", "limit": {"context": 200000, "output": 64000},
                "cost": {"input": 3, "output": 15}}}}
}"#;

/// Catálogo de muestra ya construido, listo para `App::set_catalog`.
#[cfg(test)]
pub(crate) fn picker_sample_catalog() -> Arc<Catalog> {
    Arc::new(Catalog::from_json(PICKER_SAMPLE_JSON).expect("el JSON de muestra es válido"))
}

/// Catálogo de muestra con modelos que razonan (uno admite tres niveles, otro dos) y
/// uno que no razona, para las pruebas del campo «Effort».
#[cfg(test)]
pub(super) const REASONING_SAMPLE_JSON: &str = r#"{
    "anthropic": {"id": "anthropic", "name": "Anthropic", "npm": "@ai-sdk/anthropic",
        "api": "https://api.anthropic.com/v1", "env": ["ANTHROPIC_API_KEY"],
        "models": {
            "claude-think": {"id": "claude-think", "name": "Claude Think", "tool_call": true,
                "reasoning": true,
                "reasoning_options": [{"type": "effort", "values": ["low", "medium", "high"]}]},
            "claude-mid": {"id": "claude-mid", "name": "Claude Mid", "tool_call": true,
                "reasoning": true,
                "reasoning_options": [{"type": "effort", "values": ["low", "medium"]}]},
            "claude-plain": {"id": "claude-plain", "name": "Claude Plain", "tool_call": true}}}
}"#;

/// Catálogo de muestra con razonamiento ya construido, listo para `App::set_catalog`.
#[cfg(test)]
pub(crate) fn reasoning_sample_catalog() -> Arc<Catalog> {
    Arc::new(Catalog::from_json(REASONING_SAMPLE_JSON).expect("el JSON de muestra es válido"))
}

/// Catálogo de muestra con precios: `acme` publica su `api` con barra final; `priced` tiene
/// ambos precios, `half` solo el de entrada y `plain` ninguno.
#[cfg(test)]
pub(super) const PRICED_SAMPLE_JSON: &str = r#"{
    "acme": {"id": "acme", "name": "Acme", "npm": "@ai-sdk/openai-compatible",
        "api": "https://acme.example/v1/", "env": ["ACME_KEY"],
        "models": {
            "priced": {"id": "priced", "name": "Priced", "tool_call": true,
                "cost": {"input": 3, "output": 15}},
            "half": {"id": "half", "name": "Half", "tool_call": true,
                "cost": {"input": 3}},
            "plain": {"id": "plain", "name": "Plain", "tool_call": true}}}
}"#;

/// Catálogo de muestra con precios ya construido, listo para `App::set_catalog`.
#[cfg(test)]
pub(crate) fn priced_sample_catalog() -> Arc<Catalog> {
    Arc::new(Catalog::from_json(PRICED_SAMPLE_JSON).expect("el JSON de muestra es válido"))
}

/// Catálogo de muestra con ventana publicada (200 000 tokens) para el modelo `windowed`, y un
/// modelo `bare` sin ventana. Ambos cuelgan de `https://acme.example/v1`.
#[cfg(test)]
pub(crate) fn windowed_sample_catalog() -> Arc<Catalog> {
    const WINDOWED_SAMPLE_JSON: &str = r#"{
    "acme": {"id": "acme", "name": "Acme", "npm": "@ai-sdk/openai-compatible",
        "api": "https://acme.example/v1/", "env": ["ACME_KEY"],
        "models": {
            "windowed": {"id": "windowed", "name": "Windowed", "tool_call": true,
                "limit": {"context": 200000, "output": 8192}},
            "bare": {"id": "bare", "name": "Bare", "tool_call": true}}}
}"#;
    Arc::new(Catalog::from_json(WINDOWED_SAMPLE_JSON).expect("el JSON de muestra es válido"))
}

/// Posición del campo «Effort» en `/settings`. Solo existe si el modelo admite niveles;
/// sin él, «Save» ocupa esta misma posición.
pub const SETTINGS_EFFORT_FIELD: usize = 4;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderSettings {
    pub kind: ProviderKind,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    /// Nivel de esfuerzo de razonamiento; `None` equivale a «default».
    pub effort: Option<String>,
}

/// Selector abierto sobre un campo de `/settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPicker {
    /// Proveedores del catálogo.
    Provider,
    /// Modelos del proveedor del catálogo cuya URL base coincide con la actual.
    Model,
}

/// Fila de un selector: identificador que se guarda, etiqueta visible y detalle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    pub id: String,
    pub label: String,
    pub detail: String,
}

/// Nivel del árbol de la ventana `/temas` (el panel `/ui`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemasLevel {
    /// Raíz: una fila por [`UiRow`], en el orden del panel.
    Root,
    /// Lista de efectos disponibles.
    Effects,
    /// Formulario de la lluvia Matrix.
    Matrix,
    /// Lista de presets de tema.
    Theme,
    /// Lista de indicadores de actividad.
    Spinner,
}

impl TemasLevel {
    /// Título que se muestra en el bloque de la ventana.
    pub fn title(self) -> &'static str {
        match self {
            Self::Root => "Interfaz",
            Self::Effects => "Interfaz › Efectos",
            Self::Matrix => "Interfaz › Efectos › Matrix rain",
            Self::Theme => "Interfaz › Tema",
            Self::Spinner => "Interfaz › Spinner",
        }
    }

    /// Número de filas seleccionables del nivel.
    pub fn rows(self) -> usize {
        match self {
            Self::Root => UiRow::ALL.len(),
            Self::Effects => 1,
            Self::Matrix => MATRIX_FIELDS.len(),
            Self::Theme => loomcoding_cloth::widgets::ThemePreset::ALL.len(),
            Self::Spinner => SPINNER_CHOICES.len(),
        }
    }
}

/// Indicador de actividad que el composer pinta mientras el agente trabaja.
///
/// Se elige en `/temas › Spinner` y se recuerda en `ui.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinnerChoice {
    /// Un estilo distinto en cada petición, tomado de [`SPINNER_CHOICES`].
    #[default]
    Random,
    /// La barra que gira del chasis Crush.
    Crush,
    /// Un estilo concreto del catálogo `dotmax`, por índice en
    /// `loomcoding_cloth::widgets::activity::ACTIVITY_STYLES`.
    Dotmax(usize),
}

impl SpinnerChoice {
    /// Nombre estable, el que se escribe en `ui.toml`.
    pub fn key(self) -> String {
        match self {
            Self::Random => "random".into(),
            Self::Crush => "crush".into(),
            Self::Dotmax(index) => loomcoding_cloth::widgets::activity::ACTIVITY_STYLES
                .get(index)
                .copied()
                .unwrap_or("dot-ring")
                .to_string(),
        }
    }

    /// Etiqueta legible para la lista de `/temas`.
    pub fn label(self) -> String {
        match self {
            Self::Random => "Aleatorio (uno distinto por petición)".into(),
            Self::Crush => "Crush (barra girando)".into(),
            Self::Dotmax(index) => loomcoding_cloth::widgets::activity::ACTIVITY_STYLES
                .get(index)
                .copied()
                .unwrap_or("dot-ring")
                .to_string(),
        }
    }

    /// Lee un nombre de `ui.toml`; un valor desconocido cae en `Random`.
    pub fn from_key(key: &str) -> Self {
        let key = key.trim();
        if key.eq_ignore_ascii_case("random") || key.is_empty() {
            return Self::Random;
        }
        if key.eq_ignore_ascii_case("crush") {
            return Self::Crush;
        }
        match loomcoding_cloth::widgets::activity::ACTIVITY_STYLES
            .iter()
            .position(|name| name.eq_ignore_ascii_case(key))
        {
            Some(index) => Self::Dotmax(index),
            None => Self::Random,
        }
    }

    /// Estilo de `dotmax` que toca para una petición.
    ///
    /// `Random` reparte con la semilla; el resto es fijo. `Crush` no usa
    /// `dotmax`, así que devuelve el primero solo para tener un valor.
    pub fn style_for(self, seed: u64) -> &'static str {
        match self {
            Self::Random => loomcoding_cloth::widgets::activity::pick_activity_style(seed),
            Self::Crush => loomcoding_cloth::widgets::activity::ACTIVITY_STYLES[0],
            Self::Dotmax(index) => loomcoding_cloth::widgets::activity::ACTIVITY_STYLES
                .get(index)
                .copied()
                .unwrap_or(loomcoding_cloth::widgets::activity::ACTIVITY_STYLES[0]),
        }
    }

    /// Todas las opciones que ofrece `/temas › Spinner`, en orden.
    pub fn all() -> Vec<Self> {
        SPINNER_CHOICES.to_vec()
    }
}

/// Opciones del selector de spinner: aleatorio, el de Crush y todo el catálogo.
pub const SPINNER_CHOICES: &[SpinnerChoice] = &[
    SpinnerChoice::Random,
    SpinnerChoice::Crush,
    SpinnerChoice::Dotmax(0),
    SpinnerChoice::Dotmax(1),
    SpinnerChoice::Dotmax(2),
    SpinnerChoice::Dotmax(3),
    SpinnerChoice::Dotmax(4),
];

/// Campos del formulario de Matrix rain, en orden de navegación.
pub const MATRIX_FIELDS: &[&str] = &[
    "Glifos",
    "Velocidad",
    "Densidad",
    "Longitud",
    "Color",
    "Guardar",
];

/// Forma comparable de una URL base: sin espacios, sin barras finales y en minúsculas.
pub(super) fn normalize_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// Resumen de un modelo para el selector: `ctx 200k · $3/$15` (coste por millón de tokens).
pub(super) fn model_detail(model: &CatalogModel) -> String {
    let context = model.context.map_or_else(|| "-".to_string(), format_tokens);
    let cost = match (model.input_cost, model.output_cost) {
        (Some(input), Some(output)) => format!("${input}/${output}"),
        _ => "-".to_string(),
    };
    format!("ctx {context} · {cost}")
}

impl From<&LoomcodingConfig> for ProviderSettings {
    fn from(config: &LoomcodingConfig) -> Self {
        Self {
            kind: config.provider.kind,
            model: config.model.clone(),
            base_url: config.provider.base_url.clone(),
            api_key_env: config.provider.api_key_env.clone(),
            effort: config.effort.clone(),
        }
    }
}
