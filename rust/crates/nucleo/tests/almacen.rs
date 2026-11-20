use nucleo::almacen::{AlmacenError, AlmacenJson, ClaveInvalidaError};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[tokio::test]
async fn test_almacen_escritura_atomica_cache_y_borrado() {
    let tmp = tempdir().unwrap();
    let store = AlmacenJson::new(tmp.path());

    store.set("123", json!({"a": 1})).await.unwrap();
    store.set("123", json!({"a": 2})).await.unwrap();

    let disk_content = fs::read_to_string(tmp.path().join("123.json")).unwrap();
    assert_eq!(disk_content, r#"{"a":2}"#);

    let reopened = AlmacenJson::new(tmp.path());
    assert_eq!(reopened.get("123").unwrap(), None);
    reopened.initialize().await.unwrap();
    assert_eq!(reopened.get("123").unwrap(), Some(json!({"a": 2})));
    assert_eq!(store.keys(), vec!["123"]);

    store.delete("123").await.unwrap();
    assert!(!tmp.path().join("123.json").exists());
    assert_eq!(store.keys(), Vec::<String>::new());

    assert!(matches!(store.file("../evil"), Err(ClaveInvalidaError(_))));

    let tmp_files: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert_eq!(tmp_files.len(), 0);
}

#[tokio::test]
async fn test_almacen_archivo_corrupto_se_omite_y_no_se_borra() {
    let tmp = tempdir().unwrap();
    let corrupt_file = tmp.path().join("corrupto.json");
    fs::write(&corrupt_file, "este no es un json valido {").unwrap();

    let store = AlmacenJson::new(tmp.path());
    store.initialize().await.unwrap();

    assert_eq!(store.get("corrupto").unwrap(), None);
    assert!(corrupt_file.exists());
    assert_eq!(
        fs::read_to_string(&corrupt_file).unwrap(),
        "este no es un json valido {"
    );
}

#[tokio::test]
async fn test_almacen_claves_invalidas() {
    let tmp = tempdir().unwrap();
    let store = AlmacenJson::new(tmp.path());

    let long_key = "a".repeat(65);
    for bad in &["../evil", "", "key with space", "key/slash", &long_key] {
        let err = store.get(bad).unwrap_err();
        assert_eq!(err.to_string(), format!("Invalid store key: {bad}"));

        assert!(matches!(
            store.set(bad, json!({})).await,
            Err(AlmacenError::ClaveInvalida(_))
        ));

        assert!(matches!(
            store.delete(bad).await,
            Err(AlmacenError::ClaveInvalida(_))
        ));
    }
}

#[tokio::test]
async fn test_almacen_paridad_leer_archivos_de_node() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data_dir = manifest_dir.join("tests").join("data").join("almacen");

    let store = AlmacenJson::new(&data_dir);
    store.initialize().await.unwrap();

    let mut keys = store.keys();
    keys.sort();
    assert_eq!(
        keys,
        vec!["guild_nested", "guild_simple", "special-key_123"]
    );

    assert_eq!(
        store.get("guild_simple").unwrap(),
        Some(json!({"language": "es", "volume": 80}))
    );

    assert_eq!(
        store.get("special-key_123").unwrap(),
        Some(json!({"count": 42, "enabled": true, "name": "test_store"}))
    );

    let nested = store.get("guild_nested").unwrap().unwrap();
    assert_eq!(nested["prefix"], "!");
    assert_eq!(nested["filters"]["bassboost"], true);
    assert_eq!(nested["filters"]["nightcore"], false);
    assert_eq!(nested["users"], json!(["123", "456"]));
}

#[tokio::test]
async fn test_almacen_paridad_escritura_rust_leida_por_node() {
    let tmp = tempdir().unwrap();
    let store = AlmacenJson::new(tmp.path());

    store
        .set("from_rust_1", json!({"saludo": "hola", "num": 100}))
        .await
        .unwrap();
    store
        .set(
            "from_rust_2",
            json!({"arr": [1, 2, 3], "nested": {"ok": true}}),
        )
        .await
        .unwrap();
    store.drain().await;

    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("Failed to locate project root")
        .to_path_buf();
    let script_path = project_root.join("scripts").join("almacen-paridad-ref.js");

    let output = Command::new("node")
        .arg(&script_path)
        .arg("--leer")
        .arg(tmp.path())
        .output()
        .expect("Fallo al ejecutar node para paridad");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["keys"], json!(["from_rust_1", "from_rust_2"]));
    assert_eq!(
        parsed["values"]["from_rust_1"],
        json!({"saludo": "hola", "num": 100})
    );
    assert_eq!(
        parsed["values"]["from_rust_2"],
        json!({"arr": [1, 2, 3], "nested": {"ok": true}})
    );
}

#[tokio::test]
async fn test_almacen_drain_y_escrituras_concurrentes() {
    let tmp = tempdir().unwrap();
    let store = std::sync::Arc::new(AlmacenJson::new(tmp.path()));

    let mut handles = vec![];
    for i in 0..10 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            let key = format!("k_{}", i % 3);
            s.set(&key, json!({"val": i})).await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
    store.drain().await;

    let reopened = AlmacenJson::new(tmp.path());
    reopened.initialize().await.unwrap();
    for i in 0..3 {
        let key = format!("k_{i}");
        assert!(reopened.get(&key).unwrap().is_some());
    }
}

#[tokio::test]
async fn test_almacen_keys_conserva_orden_insercion() {
    let tmp = tempdir().unwrap();
    let store = AlmacenJson::new(tmp.path());

    store.set("first", json!(1)).await.unwrap();
    store.set("second", json!(2)).await.unwrap();
    store.set("third", json!(3)).await.unwrap();

    assert_eq!(store.keys(), vec!["first", "second", "third"]);

    store.set("first", json!(10)).await.unwrap();
    assert_eq!(store.keys(), vec!["second", "third", "first"]);
}
