use nucleo::idioma::{clean, Idiomas};
use nucleo::idioma_servidor::{
    guild_language, set_guild_language, IdiomaDesconocido, LegacyLanguages,
};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn get_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("Failed to locate project root")
        .to_path_buf()
}

#[test]
fn test_clean_and_t_parity_with_node() {
    let root = get_project_root();
    let lang_dir = root.join("languages");
    let idiomas = Idiomas::cargar(&lang_dir, Some("en")).expect("Failed to load languages");

    let casos_file = root
        .join("rust")
        .join("crates")
        .join("nucleo")
        .join("tests")
        .join("data")
        .join("t-casos.json");
    let casos_str = fs::read_to_string(&casos_file).expect("Failed to read t-casos.json");
    let casos: Value = serde_json::from_str(&casos_str).expect("Invalid t-casos.json");

    for c in casos["clean"].as_array().unwrap() {
        let input = c["input"].as_str().unwrap();
        let expected = c["output"].as_str().unwrap();
        assert_eq!(clean(input), expected, "clean mismatch for {input:?}");
    }

    for c in casos["t"].as_array().unwrap() {
        let lang = c["lang"].as_str().unwrap();
        let key = c["key"].as_str().unwrap();
        let expected = c["output"].as_str().unwrap();
        let mut vars = HashMap::new();
        if let Some(map) = c["vars"].as_object() {
            for (k, v) in map {
                vars.insert(k.clone(), v.as_str().unwrap_or("").to_string());
            }
        }
        let actual = idiomas.t(lang, key, &vars);
        assert_eq!(actual, expected, "t mismatch for {lang} {key}");
    }

    for c in casos["discordLocales"].as_array().unwrap() {
        let loc = c["locale"].as_str().unwrap();
        let expected = c["output"].as_str();
        assert_eq!(
            idiomas.from_discord_locale(loc),
            expected,
            "Discord locale mismatch for {loc}"
        );
    }

    assert_eq!(idiomas.len(), 23);
    let list = idiomas.list();
    assert_eq!(list.len(), 23);
    for item in list {
        assert!(
            !item.name.is_empty(),
            "Language {} has empty name",
            item.code
        );
    }

    let loc_play = idiomas.localizations("commands.play.description");
    let ref_play = &casos["localizationsPlay"];
    for (loc, expected_val) in ref_play.as_object().unwrap() {
        assert_eq!(
            loc_play.get(loc).map(|s| s.as_str()),
            Some(expected_val.as_str().unwrap()),
            "localizationsPlay mismatch for {loc}"
        );
    }
}

#[tokio::test]
async fn test_guild_language_resolution_and_persistence() {
    let root = get_project_root();
    let lang_dir = root.join("languages");
    let idiomas = Idiomas::cargar(&lang_dir, Some("en")).expect("Failed to load languages");

    let tmp = tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();

    let legacy_root = tmp.path().join("legacy");
    fs::create_dir_all(legacy_root.join("database")).unwrap();
    fs::write(
        legacy_root.join("database").join("languages.json"),
        serde_json::json!({
            "servers": {
                "888888888888888888": { "language": "fr" },
                "777777777777777777": { "language": "xx" }
            }
        })
        .to_string(),
    )
    .unwrap();

    let legacy = LegacyLanguages::cargar(&legacy_root);
    assert_eq!(legacy.get("888888888888888888"), Some("fr"));
    assert_eq!(legacy.get("000000000000000000"), None);

    assert_eq!(
        guild_language(&idiomas, &data_dir, &legacy, None, Some("de")),
        "de"
    );
    assert_eq!(
        guild_language(&idiomas, &data_dir, &legacy, None, Some("xx")),
        "en"
    );

    assert_eq!(
        guild_language(
            &idiomas,
            &data_dir,
            &legacy,
            Some("888888888888888888"),
            Some("de")
        ),
        "fr"
    );
    assert_eq!(
        guild_language(
            &idiomas,
            &data_dir,
            &legacy,
            Some("777777777777777777"),
            Some("de")
        ),
        "de"
    );

    set_guild_language(&idiomas, &data_dir, "999999999999999999", "tr")
        .await
        .unwrap();
    assert_eq!(
        guild_language(
            &idiomas,
            &data_dir,
            &legacy,
            Some("999999999999999999"),
            Some("de")
        ),
        "tr"
    );

    assert_eq!(
        set_guild_language(&idiomas, &data_dir, "999999999999999999", "xx").await,
        Err(IdiomaDesconocido("xx".to_string()))
    );

    set_guild_language(&idiomas, &data_dir, "888888888888888888", "tr")
        .await
        .unwrap();
    assert_eq!(
        guild_language(
            &idiomas,
            &data_dir,
            &legacy,
            Some("888888888888888888"),
            Some("de")
        ),
        "tr"
    );
}
