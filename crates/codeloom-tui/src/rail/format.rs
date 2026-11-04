//! Formato de cifras: tokens, costes, precios y tiempo.

use super::*;

/// Cifra compacta de tokens: `999`, `1.2k`, `15k`, `1.5M`, `2G`. El redondeo es
/// hacia abajo (`1_999` → `1.9k`). La decimal solo aparece con una cifra entera
/// y es distinta de cero; a partir de diez se muestra solo la parte entera.
pub fn format_tokens(n: u64) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let (unidad, divisor) = if n < 1_000_000 {
        ('k', 1_000)
    } else if n < 1_000_000_000 {
        ('M', 1_000_000)
    } else {
        ('G', 1_000_000_000)
    };
    let entero = n / divisor;
    if entero < 10 {
        let decima = (n % divisor) / (divisor / 10);
        if decima > 0 {
            return format!("{entero}.{decima}{unidad}");
        }
    }
    format!("{entero}{unidad}")
}

/// Coste estimado en dólares, siempre con el prefijo `≈` de estimación: `≈ $0.0123` por
/// debajo de uno (cuatro decimales), `≈ $1.23` hasta cien y `≈ $123` desde ahí. Un valor
/// positivo pero menor que una diezmilésima se muestra como `≈ < $0.0001`. Los umbrales
/// de redondeo evitan salidas como «1.0000» o «100.00». Si `partial`, lleva un `+` final:
/// hay apuntes sin precio que no entran en la cifra.
pub fn format_cost(dollars: f64, partial: bool) -> String {
    let base = format_dollars(dollars);
    if partial {
        format!("{base}+")
    } else {
        base
    }
}

pub(super) fn format_actual_cost(dollars: f64, partial: bool) -> String {
    format_cost(dollars, partial).replacen("≈ ", "", 1)
}

/// Cifra de `format_cost` sin el marcador de parcial.
pub(super) fn format_dollars(dollars: f64) -> String {
    if dollars <= 0.0 {
        return "≈ $0".to_owned();
    }
    if dollars < 0.0001 {
        return "≈ < $0.0001".to_owned();
    }
    if dollars < 0.99995 {
        format!("≈ ${dollars:.4}")
    } else if dollars < 99.995 {
        format!("≈ ${dollars:.2}")
    } else {
        format!("≈ ${dollars:.0}")
    }
}

/// Precio por millón del modelo, p. ej. `$3/$15 /M`. Sin decimales si el precio es entero;
/// si no, hasta dos decimales sin ceros sobrantes.
pub fn format_prices(input: f64, output: f64) -> String {
    format!("{}/{} /M", price_part(input), price_part(output))
}

/// Un precio con el signo `$` delante y sin ceros sobrantes.
pub(super) fn price_part(price: f64) -> String {
    if price.fract() == 0.0 {
        format!("${price:.0}")
    } else {
        let fixed = format!("{price:.2}");
        format!("${}", fixed.trim_end_matches('0').trim_end_matches('.'))
    }
}

/// Tiempo transcurrido compacto: `7s`, `1m05s` (menos de una hora) o `1h01m`.
pub fn format_elapsed(d: Duration) -> String {
    let total = d.as_secs();
    if total < 60 {
        format!("{total}s")
    } else if total < 3_600 {
        format!("{}m{:02}s", total / 60, total % 60)
    } else {
        format!("{}h{:02}m", total / 3_600, (total % 3_600) / 60)
    }
}

/// Grupo de coste de un hijo para su línea de métricas: `format_cost` sin el espacio tras `≈`
/// (`≈$0.004`, `≈$0.004+`). `None` sin precio o si el coste es nulo, para no mostrar `≈$0`.
pub(super) fn child_cost_label(cost: Option<(f64, bool)>, estimated: bool) -> Option<String> {
    cost.filter(|(dollars, _)| *dollars > 0.0)
        .map(|(dollars, partial)| {
            if estimated {
                format_cost(dollars, partial).replacen("≈ ", "≈", 1)
            } else {
                format_actual_cost(dollars, partial)
            }
        })
}
