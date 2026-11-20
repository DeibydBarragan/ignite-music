use nucleo::cache::CacheTtl;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_cache_caducidad_y_carga_unica_concurrente() {
    let cache = CacheTtl::<String>::new(Some(30), None);
    let calls = Arc::new(AtomicUsize::new(0));

    let c1 = cache.clone();
    let calls1 = calls.clone();
    let h1 = tokio::spawn(async move {
        c1.wrap(
            "k",
            || async {
                calls1.fetch_add(1, Ordering::SeqCst);
                sleep(Duration::from_millis(10)).await;
                Ok::<String, String>("value".to_string())
            },
            None,
        )
        .await
    });

    let c2 = cache.clone();
    let calls2 = calls.clone();
    let h2 = tokio::spawn(async move {
        c2.wrap(
            "k",
            || async {
                calls2.fetch_add(1, Ordering::SeqCst);
                sleep(Duration::from_millis(10)).await;
                Ok::<String, String>("value".to_string())
            },
            None,
        )
        .await
    });

    let (res1, res2) = tokio::join!(h1, h2);
    assert_eq!(res1.unwrap().unwrap(), "value");
    assert_eq!(res2.unwrap().unwrap(), "value");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // After expiration (30ms * 1.1 max jitter is ~33ms, sleep 45ms)
    sleep(Duration::from_millis(45)).await;
    assert_eq!(cache.get("k"), None);

    // Rejected loads are not cached and can be retried
    let attempts = Arc::new(AtomicUsize::new(0));
    let a1 = attempts.clone();
    let r1 = cache
        .wrap(
            "err",
            || async {
                a1.fetch_add(1, Ordering::SeqCst);
                Err::<String, String>("x".to_string())
            },
            None,
        )
        .await;
    assert!(r1.is_err());

    let a2 = attempts.clone();
    let r2 = cache
        .wrap(
            "err",
            || async {
                a2.fetch_add(1, Ordering::SeqCst);
                Err::<String, String>("x".to_string())
            },
            None,
        )
        .await;
    assert!(r2.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_cache_barrido_sin_lectura() {
    let cache = CacheTtl::<i32>::new(Some(20), None);
    cache.set("a", 1, None);

    sleep(Duration::from_millis(35)).await;
    assert_eq!(cache.len(), 1, "entries expire lazily until swept");

    cache.sweep();
    assert_eq!(cache.len(), 0);
}

#[tokio::test]
async fn test_cache_eviccion_max_size() {
    let cache = CacheTtl::<i32>::new(Some(1000), Some(2));
    cache.set("a", 1, None);
    cache.set("b", 2, None);
    cache.set("c", 3, None);

    assert_eq!(cache.get("a"), None);
    assert_eq!(cache.get("b"), Some(2));
    assert_eq!(cache.get("c"), Some(3));
}

#[tokio::test]
async fn test_cache_sin_ttl_no_caduca() {
    let cache = CacheTtl::<&'static str>::new(None, None);
    cache.set("infinito", "valor_persistente", None);

    sleep(Duration::from_millis(50)).await;
    assert_eq!(cache.get("infinito"), Some("valor_persistente"));
    assert!(cache.has("infinito"));

    cache.sweep();
    assert_eq!(cache.get("infinito"), Some("valor_persistente"));
}

#[tokio::test]
async fn test_cache_has_delete_y_clear() {
    let cache = CacheTtl::<i32>::new(Some(500), None);
    cache.set("item1", 10, None);
    cache.set("item2", 20, None);

    assert!(cache.has("item1"));
    assert!(cache.has("item2"));
    assert!(!cache.has("item3"));

    cache.delete("item1");
    assert!(!cache.has("item1"));
    assert_eq!(cache.get("item1"), None);
    assert_eq!(cache.len(), 1);

    cache.clear();
    assert_eq!(cache.len(), 0);
    assert!(!cache.has("item2"));
}
