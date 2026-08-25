//! Tiny deterministic PRNG (xorshift64*), so runs are reproducible and we
//! avoid pulling `getrandom` into the wasm build.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1 << 24) as f32
    }

    #[inline]
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }

    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    /// Uniform direction on the unit circle.
    #[inline]
    pub fn dir(&mut self) -> [f32; 2] {
        let a = self.range(0.0, std::f32::consts::TAU);
        [a.cos(), a.sin()]
    }
}
