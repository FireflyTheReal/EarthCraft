//! Cache commands and completion polling; filesystem ownership stays in data.
use super::App;
use earthcraft_core::language::Lang;
use eframe::egui;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

const DISK_USAGE_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const CACHE_CLEAR_NOTICE_DURATION: Duration = Duration::from_secs(5);
const BYTES_PER_KIBIBYTE: u64 = 1024;
type CacheResult = Result<(), earthcraft_data::Error>;
type UsageResult = Result<u64, earthcraft_data::Error>;

struct CacheClearNotice {
    message: String,
    expires_at: Instant,
}

#[derive(Default)]
pub(super) struct CacheControls {
    clear_rx: Option<mpsc::Receiver<CacheResult>>,
    usage_rx: Option<mpsc::Receiver<UsageResult>>,
    pub(super) usage: Option<Result<u64, String>>,
    last_usage_scan: Option<Instant>,
    clear_notice: Option<CacheClearNotice>,
}
impl CacheControls {
    pub(super) fn clearing(&self) -> bool {
        self.clear_rx.is_some()
    }

    fn poll_clear_notice(
        &mut self,
        status: &mut String,
        ready: &str,
        now: Instant,
    ) -> Option<Duration> {
        let notice = self.clear_notice.as_ref()?;
        if *status != notice.message {
            self.clear_notice = None;
        } else if now >= notice.expires_at {
            *status = ready.into();
            self.clear_notice = None;
        } else {
            return Some(notice.expires_at - now);
        }
        None
    }
}

pub(super) fn cache_size_text(bytes: u64) -> String {
    for (unit, scale) in [
        ("GiB", BYTES_PER_KIBIBYTE.pow(3)),
        ("MiB", BYTES_PER_KIBIBYTE.pow(2)),
        ("KiB", BYTES_PER_KIBIBYTE),
    ] {
        if bytes >= scale {
            return format!("{:.1} {unit}", bytes as f64 / scale as f64);
        }
    }
    format!("{bytes} B")
}

impl App {
    pub(super) fn clear_disk_cache(&mut self, ctx: &egui::Context) {
        if self.cache_controls.clearing() {
            return;
        }
        self.tile_warmup.cancel();
        let (tx, rx) = mpsc::channel();
        self.cache_controls.clear_rx = Some(rx);
        self.cache_controls.usage_rx = None;
        ctx.request_repaint();
        let (cache, ctx) = (self.cache.clone(), ctx.clone());
        self.runtime.spawn(async move {
            let _ = tx.send(cache.clear_disk_cache().await);
            ctx.request_repaint();
        });
    }
    pub(super) fn poll_cache_clear(&mut self, ctx: &egui::Context) {
        if let Some(remaining) = self.cache_controls.poll_clear_notice(
            &mut self.status,
            self.lang.text("app-ready-status"),
            Instant::now(),
        ) {
            ctx.request_repaint_after(remaining);
        }
        self.poll_disk_usage();
        let Some(rx) = self.cache_controls.clear_rx.take() else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                self.cache_controls.clear_rx = Some(rx);
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("cache clearing task stopped".into()),
        };
        self.cache_controls.last_usage_scan = None;
        self.cache_controls.usage = None;
        self.cache_controls.clear_notice = None;
        match result {
            Ok(()) => {
                self.status = self.lang.text("settings-cache-cleared-status").into();
                self.cache_controls.clear_notice = Some(CacheClearNotice {
                    message: self.status.clone(),
                    expires_at: Instant::now() + CACHE_CLEAR_NOTICE_DURATION,
                });
                ctx.request_repaint_after(CACHE_CLEAR_NOTICE_DURATION);
            }
            Err(error) => {
                self.status = Lang::En.format(
                    "settings-cache-clear-error",
                    &[("error", error.to_string().into())],
                );
            }
        }
    }

    // Invoked by Settings only; scans are asynchronous and throttled, never per frame.
    pub(super) fn refresh_disk_usage(&mut self, ctx: &egui::Context) {
        if self.cache_controls.clearing() || self.cache_controls.usage_rx.is_some() {
            return;
        }
        if self
            .cache_controls
            .last_usage_scan
            .is_some_and(|last| last.elapsed() < DISK_USAGE_REFRESH_INTERVAL)
        {
            ctx.request_repaint_after(DISK_USAGE_REFRESH_INTERVAL);
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.cache_controls.usage_rx = Some(rx);
        self.cache_controls.last_usage_scan = Some(Instant::now());
        let (cache, ctx) = (self.cache.clone(), ctx.clone());
        self.runtime.spawn(async move {
            let _ = tx.send(cache.disk_usage_bytes().await);
            ctx.request_repaint();
        });
    }

    fn poll_disk_usage(&mut self) {
        let Some(rx) = self.cache_controls.usage_rx.take() else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(mpsc::TryRecvError::Empty) => {
                self.cache_controls.usage_rx = Some(rx);
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("cache measurement task stopped".into()),
        };
        self.cache_controls.usage = Some(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_clear_success_expires_at_deadline_in_both_languages() {
        let now = Instant::now();
        for lang in Lang::ALL {
            let mut status = lang.text("settings-cache-cleared-status").to_owned();
            let ready = lang.text("app-ready-status");
            let mut controls = CacheControls {
                clear_notice: Some(CacheClearNotice {
                    message: status.clone(),
                    expires_at: now + CACHE_CLEAR_NOTICE_DURATION,
                }),
                ..Default::default()
            };
            assert_eq!(
                controls.poll_clear_notice(&mut status, ready, now),
                Some(CACHE_CLEAR_NOTICE_DURATION)
            );
            assert_eq!(status, lang.text("settings-cache-cleared-status"));
            assert_eq!(
                controls.poll_clear_notice(&mut status, ready, now + CACHE_CLEAR_NOTICE_DURATION),
                None
            );
            assert_eq!(status, ready);
        }
    }

    #[test]
    fn notice_expiration_preserves_newer_messages_and_errors() {
        let now = Instant::now();
        for newer in ["Search in progress", "Settings persistence failed: fixture"] {
            let mut status = newer.to_owned();
            let mut controls = CacheControls {
                clear_notice: Some(CacheClearNotice {
                    message: Lang::En.text("settings-cache-cleared-status").into(),
                    expires_at: now,
                }),
                ..Default::default()
            };
            assert_eq!(controls.poll_clear_notice(&mut status, "Ready", now), None);
            assert_eq!(status, newer);
            assert!(controls.clear_notice.is_none());
        }
    }
}
