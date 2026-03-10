use futures::StreamExt;
use miette::IntoDiagnostic;
use postage::{sink::Sink, stream::Stream};

use super::{Format, Storage, StorageBytes};

#[cfg(feature = "serde")]
use super::{StorageTyped, serde::deserialize_bytes};

pub struct NatsStorage {
    key: String,
    rx: postage::dispatch::Receiver<async_nats::jetstream::kv::Entry>,
    store: async_nats::jetstream::kv::Store,
    #[allow(dead_code)]
    format: Format,
}

impl NatsStorage {
    pub async fn new(
        client: async_nats::Client,
        bucket: impl Into<String>,
        key: impl AsRef<str>,
        format: Format,
    ) -> miette::Result<Self> {
        let js = async_nats::jetstream::new(client);
        let store = js.get_key_value(bucket).await.into_diagnostic()?;

        let mut watch = store.watch(&key).await.into_diagnostic()?;
        let (mut tx, rx) = postage::dispatch::channel(1024);
        tokio::spawn(async move {
            while let Some(entry) = watch.next().await {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        tracing::error!("Error watching NATS KV: {e}");
                        continue;
                    }
                };
                _ = tx
                    .send(entry)
                    .await
                    .into_diagnostic()
                    .inspect_err(|e| tracing::error!("KV watcher send error: {e}"));
            }
        });

        Ok(Self {
            store,
            rx,
            key: key.as_ref().to_string(),
            format,
        })
    }
}

impl Storage for NatsStorage {
    fn path(&self) -> String {
        format!("{}/{}", self.store.name, self.key)
    }
}

#[cfg(feature = "serde")]
#[async_trait::async_trait]
impl<T: serde::de::DeserializeOwned> StorageTyped<T> for NatsStorage {
    async fn load(&self) -> miette::Result<T> {
        deserialize_bytes(&self.load_bytes().await?, self.format)
    }

    async fn watch(&self) -> Option<T> {
        let mut rx = self.rx.clone();
        match rx.recv().await {
            Some(entry) => match deserialize_bytes(&entry.value, self.format) {
                Ok(config) => Some(config),
                Err(e) => {
                    tracing::warn!("Failed to reload config file: {e}");
                    None
                }
            },
            None => {
                tracing::error!("NATS KV watch channel closed");
                return None;
            }
        }
    }
}

#[async_trait::async_trait]
impl StorageBytes for NatsStorage {
    async fn load_bytes(&self) -> miette::Result<Vec<u8>> {
        self.store
            .get(&self.key)
            .await
            .into_diagnostic()?
            .ok_or(miette::miette!(
                "Key {} not found in bucket {}",
                self.key,
                self.store.name
            ))
            .map(|b| b.to_vec())
    }

    async fn watch_bytes(&self) -> Option<Vec<u8>> {
        let mut rx = self.rx.clone();
        match rx.recv().await {
            Some(entry) => Some(entry.value.to_vec()),
            None => {
                tracing::error!("NATS KV watch channel closed");
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // use crate::config::{HotConfig, sync::storage::serde::serialize_bytes};

    // use super::*;

    // macro_rules! nats_or_skip {
    //     () => {
    //         match (
    //             std::env::var("NATS_URL"),
    //             std::env::var("NATS_USER"),
    //             std::env::var("NATS_PASSWORD"),
    //         ) {
    //             (Ok(url), Ok(user), Ok(password)) => async_nats::ConnectOptions::new()
    //                 .user_and_password(user, password)
    //                 .connect(url)
    //                 .await
    //                 .unwrap(),
    //             _ => return,
    //         }
    //     };
    // }

    //     #[tokio::test]
    //     async fn test_nats_storage_load_and_watch() {
    //         let bucket = "test";
    //         let key = "hot_config";
    //         let format = Format::Json;

    //         let client = nats_or_skip!();
    //         let kv = async_nats::jetstream::new(client.clone())
    //             .get_key_value(bucket)
    //             .await
    //             .unwrap();
    //         let mut hot_config =
    //             deserialize_bytes(&kv.get(key).await.unwrap().unwrap(), format).unwrap();

    //         let storage = NatsStorage::new(client, bucket, key, format).await.unwrap();

    //         let config = StorageTyped::<HotConfig>::load(&storage).await.unwrap();
    //         assert_eq!(config, hot_config);

    //         hot_config.ip_rate_limits_per_window += 1;
    //         kv.put(key, serialize_bytes(&hot_config, format).unwrap().into())
    //             .await
    //             .unwrap();
    //         let config = StorageTyped::<HotConfig>::watch(&storage).await;
    //         assert_eq!(config, Some(hot_config.clone()));

    //         hot_config.ip_rate_limits_per_window -= 1;
    //         kv.put(key, serialize_bytes(&hot_config, format).unwrap().into())
    //             .await
    //             .unwrap();
    //         let config = StorageTyped::<HotConfig>::watch(&storage).await;
    //         assert_eq!(config, Some(hot_config.clone()));
    //     }
}
