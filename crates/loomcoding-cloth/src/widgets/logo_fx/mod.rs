//! Catálogo y planificador de los efectos del logo de cabecera.
//!
//! Cada ráfaga elige, al azar y sin repetir el anterior, uno de los siete efectos y lo aplica
//! unos segundos sobre el rectángulo de la marca. Entre ráfagas no hay reloj: el planificador
//! publica el instante de la siguiente (`deadline`) para que el bucle despierte sin sondear.
//! No conoce el producto: recibe el instante y la semilla por parámetro.

mod draw;
mod effects;
mod rng;
mod scheduler;

pub use draw::apply;
pub use rng::Rng;
pub use scheduler::{LogoFx, LogoFxState, MAX_INTERVAL, MIN_INTERVAL};

/// Efecto del catálogo. El orden de `ALL` no tiene significado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogoEffect {
    /// Lluvia de `0` y `1` tenues sobre las celdas vacías.
    BinaryRain,
    /// Banda diagonal de brillo que recorre la marca.
    Shimmer,
    /// Foco elíptico que cruza la marca aclarando lo que cubre.
    Flashlight,
    /// Las letras se desordenan en glifos y se recomponen.
    Scramble,
    /// Desplazamientos de filas y celdas teñidas de error.
    Glitch,
    /// Las letras se desvanecen y vuelven en orden pseudoaleatorio.
    Dissolve,
    /// El brillo de toda la marca oscila.
    Pulse,
}

impl LogoEffect {
    /// Los siete efectos, en un orden fijo.
    pub const ALL: [LogoEffect; 7] = [
        LogoEffect::BinaryRain,
        LogoEffect::Shimmer,
        LogoEffect::Flashlight,
        LogoEffect::Scramble,
        LogoEffect::Glitch,
        LogoEffect::Dissolve,
        LogoEffect::Pulse,
    ];

    /// Nombre legible, para la ayuda y los registros.
    pub fn name(self) -> &'static str {
        match self {
            LogoEffect::BinaryRain => "lluvia binaria",
            LogoEffect::Shimmer => "reflejo",
            LogoEffect::Flashlight => "linterna",
            LogoEffect::Scramble => "tipográfico",
            LogoEffect::Glitch => "glitch",
            LogoEffect::Dissolve => "disolución",
            LogoEffect::Pulse => "pulso",
        }
    }
}
