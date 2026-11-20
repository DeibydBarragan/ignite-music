pub use crate::almacen_disco::*;
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{Notify, OnceCell};

pub struct AlmacenJson {
    dir: PathBuf,
    cache: RwLock<IndexMap<String, Value>>,
    key_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    active_writes: AtomicUsize,
    drain_notify: Notify,
    ready: OnceCell<()>,
}

impl AlmacenJson {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            cache: RwLock::new(IndexMap::new()),
            key_locks: Mutex::new(HashMap::new()),
            active_writes: AtomicUsize::new(0),
            drain_notify: Notify::new(),
            ready: OnceCell::new(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn file(&self, key: &str) -> Result<PathBuf, ClaveInvalidaError> {
        validate_key(key)?;
        Ok(self.dir.join(format!("{key}.json")))
    }

    pub async fn initialize(&self) -> Result<(), AlmacenError> {
        self.ready
            .get_or_init(|| async {
                let _ = tokio::fs::create_dir_all(&self.dir).await;
                let mut entries = match tokio::fs::read_dir(&self.dir).await {
                    Ok(e) => e,
                    Err(err) => {
                        tracing::warn!("Could not list {}: {}", self.dir.display(), err);
                        return;
                    }
                };
                let mut loaded = Vec::new();
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    if !name.ends_with(".json") {
                        continue;
                    }
                    let key = &name[..name.len() - 5];
                    if validate_key(key).is_err() {
                        continue;
                    }
                    let path = entry.path();
                    match tokio::fs::read_to_string(&path).await {
                        Ok(content) => match serde_json::from_str::<Value>(&content) {
                            Ok(val) => {
                                loaded.push((key.to_string(), val));
                            }
                            Err(err) => tracing::warn!("Could not read {}: {}", key, err),
                        },
                        Err(err) => tracing::warn!("Could not read {}: {}", key, err),
                    }
                }
                let mut cache = self.cache.write().unwrap();
                for (k, v) in loaded {
                    cache.entry(k).or_insert(v);
                }
            })
            .await;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<Value>, ClaveInvalidaError> {
        validate_key(key)?;
        let cache = self.cache.read().unwrap();
        match cache.get(key) {
            Some(val) if !val.is_null() => Ok(Some(val.clone())),
            _ => Ok(None),
        }
    }

    pub async fn set(&self, key: &str, value: Value) -> Result<(), AlmacenError> {
        validate_key(key)?;
        self.initialize().await?;
        {
            let mut cache = self.cache.write().unwrap();
            cache.shift_remove(key);
            cache.insert(key.to_string(), value);
        }
        self.flush(key).await
    }

    pub async fn delete(&self, key: &str) -> Result<(), AlmacenError> {
        validate_key(key)?;
        self.initialize().await?;
        {
            let mut cache = self.cache.write().unwrap();
            cache.shift_remove(key);
            cache.insert(key.to_string(), Value::Null);
        }
        self.flush(key).await
    }

    pub fn keys(&self) -> Vec<String> {
        let cache = self.cache.read().unwrap();
        cache
            .iter()
            .filter(|(_, val)| !val.is_null())
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub async fn flush(&self, key: &str) -> Result<(), AlmacenError> {
        validate_key(key)?;
        self.initialize().await?;

        let key_lock = {
            let mut map = self.key_locks.lock().unwrap();
            map.entry(key.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };

        self.active_writes.fetch_add(1, Ordering::SeqCst);
        let res = async {
            let _guard = key_lock.lock().await;
            self.write_disk(key).await
        }
        .await;

        if self.active_writes.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.drain_notify.notify_waiters();
        }

        {
            let mut map = self.key_locks.lock().unwrap();
            if let Some(entry) = map.get(key) {
                if Arc::strong_count(entry) <= 2 {
                    map.remove(key);
                }
            }
        }

        res
    }

    async fn write_disk(&self, key: &str) -> Result<(), AlmacenError> {
        let file_path = self.file(key)?;
        let value = {
            let cache = self.cache.read().unwrap();
            cache.get(key).cloned().unwrap_or(Value::Null)
        };

        if value.is_null() {
            let _ = tokio::fs::remove_file(&file_path).await;
            return Ok(());
        }

        let json_bytes = serde_json::to_vec(&value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || write_atomic_file(&dir, &file_path, &json_bytes))
            .await
            .map_err(std::io::Error::other)?
            .map_err(AlmacenError::Escritura)
    }

    pub async fn drain(&self) {
        let _ = self.initialize().await;
        loop {
            if self.active_writes.load(Ordering::SeqCst) == 0 {
                break;
            }
            let notified = self.drain_notify.notified();
            if self.active_writes.load(Ordering::SeqCst) == 0 {
                break;
            }
            notified.await;
        }
    }
}
