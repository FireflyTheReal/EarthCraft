//! One application-owned runtime for network and filesystem I/O.
use std::future::Future;
pub struct ServiceRuntime {
    runtime: tokio::runtime::Runtime,
}
impl ServiceRuntime {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("earthcraft-io")
                .enable_all()
                .build()?,
        })
    }
    pub fn handle(&self) -> &tokio::runtime::Handle {
        self.runtime.handle()
    }
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }
}
