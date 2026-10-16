//! Explicit imagery budgets; applied when the desktop constructs its cache.
use serde::Serialize;
const BYTES_PER_MIB: u64 = 1024 * 1024;
pub const MIN_CACHE_MIB: u32 = 32;
pub const MAX_DECODED_CACHE_MIB: u32 = 2048;
pub const MAX_DISK_CACHE_MIB: u32 = 16384;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CachePreferences {
    pub decoded_mib: u32,
    pub disk_mib: u32,
}
impl Default for CachePreferences {
    fn default() -> Self {
        Self {
            decoded_mib: 256,
            disk_mib: 128,
        }
    }
}
impl CachePreferences {
    pub fn decoded_bytes(self) -> usize {
        (u64::from(self.decoded_mib) * BYTES_PER_MIB) as usize
    }
    pub fn disk_bytes(self) -> u64 {
        u64::from(self.disk_mib) * BYTES_PER_MIB
    }
    pub(super) fn apply(&mut self, value: &serde_json::Value) {
        for (key, target, max) in [
            ("decoded_mib", &mut self.decoded_mib, MAX_DECODED_CACHE_MIB),
            ("disk_mib", &mut self.disk_mib, MAX_DISK_CACHE_MIB),
        ] {
            if let Some(number) = value.get(key).and_then(|v| v.as_u64()) {
                *target = number.clamp(u64::from(MIN_CACHE_MIB), u64::from(max)) as u32;
            }
        }
    }
}
