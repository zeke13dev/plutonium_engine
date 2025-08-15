//! Deterministic RNG service with per-stream generators.
//! Uses SplitMix64 for seeding and xorshift64* for the stream PRNG.
//! Also supports deriving streams from human-readable names via a fast FNV-1a 64-bit hash.

#[derive(Debug, Clone, Copy)]
pub struct RngService {
    base_seed: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct RngStream {
    state: u64,
}

/// Opaque stream identifier; typically derived from a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RngStreamId(pub u64);

impl RngService {
    pub fn with_seed(seed: u64) -> Self {
        Self { base_seed: seed }
    }

    pub fn derive_stream(&self, stream_id: u64) -> RngStream {
        let seed = splitmix64(self.base_seed ^ stream_id);
        RngStream { state: seed.max(1) }
    }

    /// Derive a stream from an opaque ID type.
    pub fn derive(&self, id: RngStreamId) -> RngStream {
        self.derive_stream(id.0)
    }

    /// Derive a stream from a human-readable name using FNV-1a 64 hashing.
    pub fn derive_stream_by_name(&self, name: &str) -> RngStream {
        let id = fnv1a64(name.as_bytes());
        self.derive_stream(id)
    }
}

impl RngStream {
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(2685821657736338717)
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() & 0xFFFF_FFFF) as u32
    }

    #[inline]
    pub fn next_f32_01(&mut self) -> f32 {
        // 24-bit mantissa precision uniform in [0,1)
        let v = (self.next_u32() >> 8) as f32;
        v / (1u32 << 24) as f32
    }

    #[inline]
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32_01()
    }

    pub fn shuffle<T>(&mut self, data: &mut [T]) {
        for i in (1..data.len()).rev() {
            let j = (self.next_u32() as usize) % (i + 1);
            data.swap(i, j);
        }
    }
}

#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z ^= z >> 30;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline]
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001B3;
    let mut hash = OFFSET;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
