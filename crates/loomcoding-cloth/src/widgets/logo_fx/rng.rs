//! Generador mínimo (xorshift64*) con semilla inyectable. Sin `rand`: las pruebas son deterministas.

/// Semilla que sustituye a la nula, porque xorshift no sale de cero.
const FALLBACK_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Generador pseudoaleatorio de 64 bits. Dos instancias con la misma semilla dan la misma secuencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// Crea el generador. La semilla 0 se sustituye por una constante.
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { FALLBACK_SEED } else { seed })
    }

    /// Siguiente valor de 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Valor en `0..n`. Con `n == 0` devuelve 0.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn misma_semilla_misma_secuencia() {
        let mut a = Rng::new(99);
        let mut b = Rng::new(99);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn semilla_cero_no_queda_atascada() {
        let mut r = Rng::new(0);
        let primeros: Vec<u64> = (0..8).map(|_| r.next_u64()).collect();
        assert!(primeros.iter().all(|&v| v != 0));
        assert!(primeros.windows(2).any(|w| w[0] != w[1]));
    }

    #[test]
    fn below_respeta_el_limite() {
        let mut r = Rng::new(5);
        for _ in 0..1000 {
            assert!(r.below(7) < 7);
        }
        assert_eq!(r.below(0), 0);
    }
}
