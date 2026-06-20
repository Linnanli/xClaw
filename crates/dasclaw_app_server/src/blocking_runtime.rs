use std::future::Future;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use tokio::runtime::{Builder, Runtime};

use crate::AppServerError;

type RuntimeTask = Box<dyn FnOnce(&Runtime) + Send + 'static>;

#[derive(Clone)]
pub(crate) struct BlockingTokioRuntime {
    inner: Arc<BlockingTokioRuntimeInner>,
    service_name: &'static str,
}

struct BlockingTokioRuntimeInner {
    sender: Mutex<Option<mpsc::Sender<RuntimeTask>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl BlockingTokioRuntime {
    pub(crate) fn new(
        worker_name: &'static str,
        service_name: &'static str,
    ) -> Result<Self, AppServerError> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                AppServerError::capability_unavailable(
                    service_name,
                    format!("{service_name} runtime unavailable: {error}"),
                )
            })?;
        let (sender, receiver) = mpsc::channel::<RuntimeTask>();
        let worker = thread::Builder::new()
            .name(worker_name.to_string())
            .spawn(move || {
                for task in receiver {
                    task(&runtime);
                }
            })
            .map_err(|error| {
                AppServerError::capability_unavailable(
                    service_name,
                    format!("{service_name} runtime worker unavailable: {error}"),
                )
            })?;
        Ok(Self {
            inner: Arc::new(BlockingTokioRuntimeInner {
                sender: Mutex::new(Some(sender)),
                worker: Mutex::new(Some(worker)),
            }),
            service_name,
        })
    }

    pub(crate) fn block_on<T>(
        &self,
        operation: &'static str,
        future: impl Future<Output = Result<T, AppServerError>> + Send + 'static,
    ) -> Result<T, AppServerError>
    where
        T: Send + 'static,
    {
        let sender = self.sender(operation)?;
        let (result_sender, result_receiver) = mpsc::channel();
        let service_name = self.service_name;
        sender
            .send(Box::new(move |runtime| {
                let result = panic::catch_unwind(AssertUnwindSafe(|| runtime.block_on(future)))
                    .unwrap_or_else(|_| {
                        Err(AppServerError::service_degraded(
                            service_name,
                            format!("{operation} panicked"),
                        ))
                    });
                let _ = result_sender.send(result);
            }))
            .map_err(|_| {
                AppServerError::capability_unavailable(
                    self.service_name,
                    format!("{operation} runtime worker is unavailable"),
                )
            })?;
        result_receiver.recv().map_err(|_| {
            AppServerError::capability_unavailable(
                self.service_name,
                format!("{operation} runtime worker stopped before returning"),
            )
        })?
    }

    fn sender(&self, operation: &'static str) -> Result<mpsc::Sender<RuntimeTask>, AppServerError> {
        self.inner
            .sender
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_ref()
            .cloned()
            .ok_or_else(|| {
                AppServerError::capability_unavailable(
                    self.service_name,
                    format!("{operation} runtime is shut down"),
                )
            })
    }
}

impl Drop for BlockingTokioRuntimeInner {
    fn drop(&mut self) {
        self.sender
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
        {
            let _ = worker.join();
        }
    }
}
