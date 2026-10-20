//! Tile-only disk persistence and coordination with explicit cache clearing.
use super::{Provider, TileKey, unkey};
use crate::wayback::wayback_release;
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

mod mbtiles;
pub(super) const DEFAULT_DISK_BUDGET_BYTES: u64 = 128 * 1024 * 1024;

pub(super) struct DiskCache {
    tilesets: mbtiles::Mbtiles,
    root: PathBuf,
    epoch: AtomicU64,
    gate: tokio::sync::RwLock<()>,
}

impl DiskCache {
    #[cfg(test)]
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            tilesets: mbtiles::Mbtiles::new(root.clone()),
            root,
            epoch: AtomicU64::new(0),
            gate: tokio::sync::RwLock::new(()),
        }
    }
    pub(super) fn with_budget(root: PathBuf, bytes: u64) -> Self {
        Self {
            tilesets: mbtiles::Mbtiles::with_limit(root.clone(), bytes),
            root,
            epoch: AtomicU64::new(0),
            gate: tokio::sync::RwLock::new(()),
        }
    }
    pub(super) fn budget_bytes(&self) -> u64 {
        self.tilesets.limit()
    }
    pub(super) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }
    pub(super) fn path(&self, key: TileKey) -> Result<PathBuf, crate::Error> {
        let (provider, zoom, x, y, release) = unkey(key);
        let mut base = self.root.join(provider.dir());
        if provider == Provider::Wayback {
            base = base.join(
                wayback_release(release)
                    .ok_or("wayback: release not loaded")?
                    .id
                    .to_string(),
            );
        }
        Ok(base
            .join(zoom.to_string())
            .join(x.to_string())
            .join(format!("{y}.tile")))
    }
    pub(super) async fn read(&self, path: &Path) -> Option<Vec<u8>> {
        let _guard = self.gate.read().await;
        self.tilesets.read(path).await.ok().flatten()
    }
    #[cfg(test)]
    pub(super) async fn write(
        &self,
        path: &Path,
        bytes: &[u8],
        epoch: u64,
    ) -> Result<(), crate::Error> {
        self.write_owned(path, bytes.to_vec(), epoch).await
    }
    pub(super) async fn write_owned(
        &self,
        path: &Path,
        bytes: Vec<u8>,
        epoch: u64,
    ) -> Result<(), crate::Error> {
        let _guard = self.gate.read().await;
        if epoch != self.epoch() {
            return Ok(());
        }
        self.tilesets.write_owned(path, bytes).await
    }
    pub(super) async fn invalidate(&self, path: &Path) -> Result<(), crate::Error> {
        let _guard = self.gate.read().await;
        self.tilesets.invalidate(path).await
    }
    pub(super) async fn clear(&self) -> Result<(), crate::Error> {
        let _guard = self.gate.write().await;
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.tilesets.clear().await
    }
    pub(super) async fn usage_bytes(&self) -> Result<u64, crate::Error> {
        let _guard = self.gate.read().await;
        self.tilesets.usage().await
    }
}

// File lengths are a snapshot of encoded cache data, excluding filesystem
// allocation overhead. Never follow links into unrelated directories.
fn count_file_bytes(root: &Path) -> io::Result<u64> {
    let mut pending: Vec<PathBuf> = Provider::CACHE_ALL
        .iter()
        .map(|provider| root.join(provider.dir()))
        .collect();
    let mut bytes = 0u64;
    while let Some(path) = pending.pop() {
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path)? {
                pending.push(entry?.path());
            }
        } else if metadata.is_file() {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
