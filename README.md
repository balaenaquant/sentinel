# sentinel

A Rust library for watching files and storage backends for changes, with support for typed deserialization.

`sentinel` provides a unified, async trait-based interface for loading and watching content from different storage backends. Currently supported backends are the **local filesystem** (Linux and macOS) and **NATS JetStream Key-Value**.

## Features

- Async-first, built on [Tokio](https://tokio.rs)
- Unified `StorageBytes` trait for raw byte access across all backends
- Optional `StorageTyped<T>` trait for automatic deserialization into your own types
- Supports **JSON** and **YAML** formats (via `serde` feature)
- Local filesystem watching via [`notify`](https://crates.io/crates/notify)
- NATS KV watching via [`async-nats`](https://crates.io/crates/async-nats)
- Structured, human-readable errors via [`miette`](https://crates.io/crates/miette)

## Installation

Add `sentinel` to your `Cargo.toml`:

```toml
[dependencies]
sentinel = { git = "https://github.com/balaenaquant/sentinel" }
```

### Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `fs`    | yes     | Local filesystem backend using `notify` |
| `nats`  | no      | NATS JetStream KV backend using `async-nats` |
| `serde` | no      | Typed deserialization via `serde` (JSON + YAML) |

To enable all features:

```toml
sentinel = { git = "https://github.com/balaenaquant/sentinel", features = ["fs", "nats", "serde"] }
```

## Usage

### Raw bytes — local file

```rust
use sentinel::file::FileWatcher;
use sentinel::StorageBytes;

#[tokio::main]
async fn main() -> miette::Result<()> {
    let watcher = FileWatcher::new("config.json".into())?;

    // Load the current contents
    let bytes = watcher.load_bytes().await?;
    println!("Loaded {} bytes", bytes.len());

    // Block until the next change, then return the new contents
    if let Some(bytes) = watcher.watch_bytes().await {
        println!("File changed: {} bytes", bytes.len());
    }

    Ok(())
}
```

### Typed deserialization — local file

Requires the `serde` feature. The format is inferred from the file extension (`.json`, `.yaml`, or `.yml`).

```rust
use serde::Deserialize;
use sentinel::file::FileWatcher;
use sentinel::StorageTyped;

#[derive(Debug, Deserialize)]
struct Config {
    host: String,
    port: u16,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let watcher = FileWatcher::new("config.yaml".into())?;

    // Load and deserialize
    let config: Config = watcher.load().await?;
    println!("host={} port={}", config.host, config.port);

    // Watch for the next change and deserialize the new value
    loop {
        if let Some(config) = watcher.watch::<Config>().await {
            println!("Config reloaded: host={} port={}", config.host, config.port);
        }
    }
}
```

### NATS JetStream KV

Requires the `nats` feature. The `Format` must be specified explicitly because NATS KV entries carry no file extension.

```rust
use sentinel::nats::NatsStorage;
use sentinel::StorageBytes;
use sentinel::Format;

#[tokio::main]
async fn main() -> miette::Result<()> {
    let client = async_nats::connect("nats://localhost:4222").await?;
    let storage = NatsStorage::new(client, "my-bucket", "config-key", Format::Json).await?;

    // Load current value
    let bytes = storage.load_bytes().await?;
    println!("Loaded {} bytes", bytes.len());

    // Block until the next KV update
    if let Some(bytes) = storage.watch_bytes().await {
        println!("Key updated: {} bytes", bytes.len());
    }

    Ok(())
}
```

With typed deserialization (requires both `nats` and `serde` features):

```rust
use serde::Deserialize;
use sentinel::nats::NatsStorage;
use sentinel::StorageTyped;
use sentinel::Format;

#[derive(Debug, Deserialize)]
struct Config {
    workers: usize,
    timeout_ms: u64,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let client = async_nats::connect("nats://localhost:4222").await?;
    let storage = NatsStorage::new(client, "my-bucket", "config-key", Format::Json).await?;

    let config: Config = storage.load().await?;
    println!("workers={}", config.workers);

    loop {
        if let Some(config) = storage.watch::<Config>().await {
            println!("Config updated: workers={}", config.workers);
        }
    }
}
```

## Traits

### `StorageBytes`

Provides raw byte access, available on all backends without the `serde` feature.

```rust
#[async_trait]
pub trait StorageBytes {
    async fn load_bytes(&self) -> miette::Result<Vec<u8>>;
    async fn watch_bytes(&self) -> Option<Vec<u8>>;
}
```

- `load_bytes` — reads the current value immediately.
- `watch_bytes` — waits for the next change and returns the new value, or `None` if the watch channel has closed.

### `StorageTyped<T>`

Extends `StorageBytes` with automatic deserialization. Requires the `serde` feature and `T: serde::de::DeserializeOwned`.

```rust
#[async_trait]
pub trait StorageTyped<T: DeserializeOwned> {
    async fn load(&self) -> miette::Result<T>;
    async fn watch(&self) -> Option<T>;
}
```

- `load` — reads and deserializes the current value.
- `watch` — waits for the next change and deserializes it. Returns `None` on channel close or deserialization error (a warning is logged).

### `Storage`

A simple trait for identifying the storage location.

```rust
pub trait Storage {
    fn path(&self) -> String;
}
```

## License

MIT — see [LICENSE](LICENSE) for details.

