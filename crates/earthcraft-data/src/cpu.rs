//! Bounded CPU services, independent of Tokio's filesystem/DNS blocking workers.
use std::sync::OnceLock;

const CPU_THREADS_PER_SERVICE: usize = 2;

pub(crate) enum Service {
    Features,
    Tiles,
    Files,
}
impl Service {
    fn pool(&self) -> Result<&'static rayon::ThreadPool, crate::Error> {
        static FEATURES: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
        static FILES: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
        static TILES: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
        let (slot, name, threads) = match self {
            Self::Features => (&FEATURES, "earthcraft-features", CPU_THREADS_PER_SERVICE),
            Self::Tiles => (&TILES, "earthcraft-decode", CPU_THREADS_PER_SERVICE),
            Self::Files => (&FILES, "earthcraft-files", 1),
        };
        slot.get_or_init(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(move |index| format!("{name}-{index}"))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| crate::Error::from(error.clone()))
    }
}

pub(crate) async fn run<T: Send + 'static>(
    service: Service,
    work: impl FnOnce() -> Result<T, crate::Error> + Send + 'static,
) -> Result<T, crate::Error> {
    #[cfg(feature = "profiling")]
    let queued = std::time::Instant::now();
    #[cfg(feature = "profiling")]
    let (queue_name, work_name) = match service {
        Service::Features => ("features.queue", "features.work"),
        Service::Tiles => ("decode.queue", "decode.work"),
        Service::Files => ("files.queue", "files.work"),
    };
    let (send, receive) = tokio::sync::oneshot::channel();
    service.pool()?.spawn(move || {
        #[cfg(feature = "profiling")]
        crate::profiling::Span::elapsed(queue_name, queued, Default::default());
        #[cfg(feature = "profiling")]
        let _profile = crate::profiling::Span::new(work_name);
        let _ = send.send(work());
    });
    receive
        .await
        .map_err(|_| crate::Error::from("CPU service stopped before returning its result"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busy_feature_workers_do_not_block_tiles_or_filesystem() {
        let runtime = crate::runtime::ServiceRuntime::new().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        for _ in 0..2 {
            let (barrier, started) = (barrier.clone(), started_tx.clone());
            runtime.spawn(async move {
                run(Service::Features, move || {
                    started.send(()).unwrap();
                    barrier.wait();
                    Ok(())
                })
                .await
                .unwrap();
            });
        }
        for _ in 0..2 {
            started_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
        }
        let result = runtime.handle().block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let tile = run(Service::Tiles, || Ok(42)).await?;
                tokio::fs::metadata(std::env::temp_dir()).await?;
                Ok::<_, crate::Error>(tile)
            })
            .await
        });
        // Release workers even on assertion failure.
        barrier.wait();
        assert_eq!(result.unwrap().unwrap(), 42);
    }
}
