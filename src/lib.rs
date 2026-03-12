use color_eyre::eyre;

#[cfg(feature = "fs")]
pub mod file;
#[cfg(feature = "nats")]
pub mod nats;

#[cfg(feature = "serde")]
pub mod serde;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Yaml,
    Toml,
    Json,
}

pub trait Storage {
    fn path(&self) -> String;
}

#[cfg(feature = "serde")]
#[async_trait::async_trait]
pub trait StorageTyped<T: ::serde::de::DeserializeOwned> {
    async fn load(&self) -> eyre::Result<T>;

    async fn watch(&self) -> Option<T>;
}

#[async_trait::async_trait]
pub trait StorageBytes {
    async fn load_bytes(&self) -> eyre::Result<Vec<u8>>;

    async fn watch_bytes(&self) -> Option<Vec<u8>>;
}
