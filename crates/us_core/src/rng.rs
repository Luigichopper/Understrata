/// SplitMix64 deterministic pseudo-random generator and integer hashing.
/// Fast, robust 64-bit state generator ideal for procedural generation and reproducible seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    /// Initialize with a 64-bit seed.
    #[inline]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Advances the generator and produces the next pseudorandom `u64`.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Produces a pseudorandom `u32`.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Produces a pseudorandom `f32` in the half-open interval `[0.0, 1.0)`.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / 4294967296.0
    }

    /// Produces a pseudorandom `f32` in the range `[min, max)`.
    #[inline]
    pub fn next_range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// Produces an integer in the range `[min, max]`.
    #[inline]
    pub fn next_range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let diff = (max - min + 1) as u32;
        min + (self.next_u32() % diff) as i32
    }
}

/// Hashes two 64-bit integers deterministically into a single `u64`.
#[inline]
pub const fn hash64_2(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E3779B97F4A7C15) ^ b.wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 30)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

/// Hashes 3 integer coordinates and a seed into a single `u64`.
#[inline]
pub const fn hash_coords3(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let ux = x as u64;
    let uy = y as u64;
    let uz = z as u64;
    let mut h = seed.wrapping_add(ux.wrapping_mul(0x9E3779B97F4A7C15));
    h = h ^ (uy.wrapping_mul(0xC6A4A7935BD1E995));
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    h = h ^ (uz.wrapping_mul(0x94D049BB133111EB));
    h = (h ^ (h >> 27)).wrapping_mul(0x9E3779B97F4A7C15);
    h ^ (h >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism() {
        let mut rng1 = Rng64::new(12345);
        let mut rng2 = Rng64::new(12345);
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_f32_range() {
        let mut rng = Rng64::new(42);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
