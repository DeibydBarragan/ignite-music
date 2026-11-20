use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use tokio::sync::broadcast;

pub trait Sweeper: Send + Sync {
    fn sweep(&self);
}

static LIVE_CACHES: LazyLock<Mutex<Vec<Weak<dyn Sweeper>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
static SWEEPER_STARTED: AtomicBool = AtomicBool::new(false);

fn ensure_sweeper_task() {
    if !SWEEPER_STARTED.load(Ordering::SeqCst) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if !SWEEPER_STARTED.swap(true, Ordering::SeqCst) {
                handle.spawn(async {
                    let mut interval =
                        tokio::time::interval(std::time::Duration::from_millis(60_000));
                    interval.tick().await;
                    loop {
                        interval.tick().await;
                        let mut list = LIVE_CACHES.lock().unwrap();
                        list.retain(|weak| {
                            if let Some(target) = weak.upgrade() {
                                target.sweep();
                                true
                            } else {
                                false
                            }
                        });
                    }
                });
            }
        }
    }
}

fn register_sweeper(cache: Weak<dyn Sweeper>) {
    LIVE_CACHES.lock().unwrap().push(cache);
    ensure_sweeper_task();
}

struct CacheEntry<V> {
    value: V,
    expires: Option<std::time::Instant>,
}

struct CacheInner<V> {
    default_ttl_ms: Option<u64>,
    max_size: usize,
    entries: Mutex<IndexMap<String, CacheEntry<V>>>,
    pending: Mutex<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>,
}

impl<V: Clone + Send + Sync + 'static> Sweeper for CacheInner<V> {
    fn sweep(&self) {
        let now = std::time::Instant::now();
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, entry| match entry.expires {
            Some(exp) => exp > now,
            None => true,
        });
    }
}

#[derive(Clone)]
pub struct CacheTtl<V> {
    inner: Arc<CacheInner<V>>,
}

impl<V: Clone + Send + Sync + 'static> CacheTtl<V> {
    pub fn new(ttl_ms: Option<u64>, max_size: Option<usize>) -> Self {
        let inner = Arc::new(CacheInner {
            default_ttl_ms: ttl_ms,
            max_size: max_size.unwrap_or(1000),
            entries: Mutex::new(IndexMap::new()),
            pending: Mutex::new(HashMap::new()),
        });
        let sweeper_arc: Arc<dyn Sweeper> = inner.clone();
        register_sweeper(Arc::downgrade(&sweeper_arc));
        Self { inner }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let mut entries = self.inner.entries.lock().unwrap();
        if let Some(entry) = entries.get(key) {
            if let Some(exp) = entry.expires {
                if std::time::Instant::now() >= exp {
                    entries.shift_remove(key);
                    return None;
                }
            }
            return Some(entry.value.clone());
        }
        None
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn set(&self, key: &str, value: V, ttl_ms: Option<u64>) {
        let effective_ttl = ttl_ms.or(self.inner.default_ttl_ms);
        let expires = effective_ttl.map(|ms| {
            let jitter = 0.9 + 0.2 * rand::random::<f64>();
            let duration_ms = (ms as f64 * jitter).round() as u64;
            std::time::Instant::now() + std::time::Duration::from_millis(duration_ms)
        });

        let mut entries = self.inner.entries.lock().unwrap();
        entries.shift_remove(key);
        entries.insert(key.to_string(), CacheEntry { value, expires });
        while entries.len() > self.inner.max_size {
            entries.shift_remove_index(0);
        }
    }

    pub fn delete(&self, key: &str) {
        let mut entries = self.inner.entries.lock().unwrap();
        entries.shift_remove(key);
    }

    pub fn clear(&self) {
        let mut entries = self.inner.entries.lock().unwrap();
        entries.clear();
        let mut pending = self.inner.pending.lock().unwrap();
        pending.clear();
    }

    pub fn sweep(&self) {
        self.inner.sweep();
    }

    pub fn len(&self) -> usize {
        let entries = self.inner.entries.lock().unwrap();
        entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub async fn wrap<F, Fut, E>(&self, key: &str, loader: F, ttl_ms: Option<u64>) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V, E>>,
        E: Clone + Send + Sync + 'static,
    {
        self.get_or_load(key, loader, ttl_ms).await
    }

    pub async fn get_or_load<F, Fut, E>(
        &self,
        key: &str,
        loader: F,
        ttl_ms: Option<u64>,
    ) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V, E>>,
        E: Clone + Send + Sync + 'static,
    {
        ensure_sweeper_task();
        if let Some(val) = self.get(key) {
            return Ok(val);
        }

        let maybe_rx = {
            let mut pending = self.inner.pending.lock().unwrap();
            if let Some(any_tx) = pending.get(key) {
                if let Ok(tx) = any_tx.clone().downcast::<broadcast::Sender<Result<V, E>>>() {
                    Some(tx.subscribe())
                } else {
                    None
                }
            } else {
                let (tx, _rx) = broadcast::channel::<Result<V, E>>(16);
                pending.insert(key.to_string(), Arc::new(tx));
                None
            }
        };

        if let Some(mut rx) = maybe_rx {
            if let Ok(res) = rx.recv().await {
                return res;
            }
            if let Some(val) = self.get(key) {
                return Ok(val);
            }
        }

        struct PendingGuard<'a> {
            pending: &'a Mutex<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>,
            key: &'a str,
        }
        impl<'a> Drop for PendingGuard<'a> {
            fn drop(&mut self) {
                let mut pending = self.pending.lock().unwrap();
                pending.remove(self.key);
            }
        }

        let _guard = PendingGuard {
            pending: &self.inner.pending,
            key,
        };

        let result = loader().await;

        if let Ok(ref val) = result {
            self.set(key, val.clone(), ttl_ms);
        }

        let tx_opt = {
            let pending = self.inner.pending.lock().unwrap();
            pending.get(key).and_then(|any_tx| {
                any_tx
                    .clone()
                    .downcast::<broadcast::Sender<Result<V, E>>>()
                    .ok()
            })
        };

        if let Some(tx) = tx_opt {
            let _ = tx.send(result.clone());
        }

        result
    }
}
