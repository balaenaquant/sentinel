use std::path::PathBuf;

use miette::IntoDiagnostic;
use notify::{
    EventKind, Watcher,
    event::{DataChange, ModifyKind},
};
use postage::{sink::Sink, stream::Stream};

use super::{Storage, StorageBytes};

#[cfg(feature = "serde")]
use super::{Format, StorageTyped, serde::deserialize_bytes};

pub struct FileWatcher {
    path: PathBuf,
    rx: postage::dispatch::Receiver<()>,

    #[allow(unused)]
    watcher: notify::RecommendedWatcher,
}
impl FileWatcher {
    pub fn new(path: PathBuf) -> miette::Result<Self> {
        let (mut tx, rx) = postage::dispatch::channel(1024);

        let mut watcher = notify::RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    tracing::debug!("File change detected: {event:?}");
                    if let EventKind::Modify(ModifyKind::Data(
                        DataChange::Content | DataChange::Any,
                    )) = event.kind
                    {
                        _ = tx
                            .try_send(())
                            .inspect_err(|e| tracing::error!("File watch send error: {e}"));
                    }
                }
                Err(e) => tracing::error!("File watch error: {e}"),
            },
            notify::Config::default(),
        )
        .into_diagnostic()?;
        watcher
            .watch(&path, notify::RecursiveMode::NonRecursive)
            .into_diagnostic()?;

        Ok(Self { path, watcher, rx })
    }

    pub fn blocking_load_bytes(&self) -> miette::Result<Vec<u8>> {
        std::fs::read(&self.path).into_diagnostic()
    }
}

impl Storage for FileWatcher {
    fn path(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

#[cfg(feature = "serde")]
#[async_trait::async_trait]
impl<T: ::serde::de::DeserializeOwned> StorageTyped<T> for FileWatcher {
    async fn load(&self) -> miette::Result<T> {
        let contents = tokio::fs::read(&self.path).await.into_diagnostic()?;

        let ext = self
            .path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(miette::miette!("File has no extension: {:?}", self.path))?;

        if ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml") {
            deserialize_bytes(&contents, Format::Yaml)
        } else if ext.eq_ignore_ascii_case("json") {
            deserialize_bytes(&contents, Format::Json)
        } else {
            miette::bail!("Unsupported file extension: {}", ext);
        }
    }

    async fn watch(&self) -> Option<T> {
        let mut rx = self.rx.clone();
        match rx.recv().await {
            Some(()) => {
                return match self.load().await {
                    Ok(config) => Some(config),
                    Err(e) => {
                        tracing::warn!("Failed to reload config file: {e}");
                        None
                    }
                };
            }
            None => {
                tracing::error!("File watch channel closed");
                return None;
            }
        }
    }
}

#[async_trait::async_trait]
impl StorageBytes for FileWatcher {
    async fn load_bytes(&self) -> miette::Result<Vec<u8>> {
        tokio::fs::read(&self.path).await.into_diagnostic()
    }

    async fn watch_bytes(&self) -> Option<Vec<u8>> {
        let mut rx = self.rx.clone();
        match rx.recv().await {
            Some(()) => {
                return match self.load_bytes().await {
                    Ok(bytes) => Some(bytes),
                    Err(e) => {
                        tracing::warn!("Failed to reload config file bytes: {e}");
                        None
                    }
                };
            }
            None => {
                tracing::error!("File watch channel closed");
                return None;
            }
        }
    }
}
