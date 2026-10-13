//! Tile-only disk persistence and coordination with explicit cache clearing.
use super::{unkey, Provider, TileKey};
use crate::wayback::wayback_release;
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) struct DiskCache {
    root: PathBuf,
    epoch: AtomicU64,
    gate: tokio::sync::RwLock<()>,
}

impl DiskCache {
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            root,
            epoch: AtomicU64::new(0),
            gate: tokio::sync::RwLock::new(()),
        }
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
        tokio::fs::read(path).await.ok()
    }
    pub(super) async fn write(&self, path: &Path, bytes: &[u8], epoch: u64) -> io::Result<()> {
        let _guard = self.gate.read().await;
        // Jobs admitted before clearing may still finish decoding into RAM,
        // but must not restore their downloads to disk after the clear.
        if epoch != self.epoch() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, bytes).await
    }
    pub(super) async fn clear(&self) -> io::Result<()> {
        let _guard = self.gate.write().await;
        self.epoch.fetch_add(1, Ordering::Relaxed);
        // Only owned provider directories: settings, language, boundary data,
        // and personal files elsewhere in the cache root are preserved.
        for provider in Provider::ALL {
            let path = self.root.join(provider.dir());
            let metadata = match tokio::fs::symlink_metadata(&path).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if metadata.file_type().is_symlink() {
                tokio::fs::remove_file(path).await?;
            } else {
                tokio::fs::remove_dir_all(path).await?;
            }
        }
        Ok(())
    }
    pub(super) async fn usage_bytes(&self) -> Result<u64, crate::Error> {
        let _guard = self.gate.read().await;
        let root = self.root.clone();
        Ok(tokio::task::spawn_blocking(move || count_file_bytes(&root)).await??)
    }
}

// File lengths are a snapshot of encoded cache data, excluding filesystem
// allocation overhead. Never follow links into unrelated directories.
fn count_file_bytes(root: &Path) -> io::Result<u64> {
    let mut pending: Vec<PathBuf> = Provider::ALL
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
