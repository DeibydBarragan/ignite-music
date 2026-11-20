use nucleo::idioma::{lookup, Idiomas};
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn get_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("Failed to locate project root")
        .to_path_buf()
}

fn collect_source_files(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str != "node_modules" && name_str != "target" && name_str != ".git" {
                    collect_source_files(&path, extensions, out);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    out.push(path);
                }
            }
        }
    }
}

static PLACEHOLDER_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\{(\w+)\}").unwrap());

fn extract_placeholders(text: &str) -> HashSet<String> {
    PLACEHOLDER_RE
        .captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

#[test]
fn test_all_23_languages_load_and_have_names() {
    let root = get_project_root();
    let lang_dir = root.join("languages");
    let idiomas = Idiomas::cargar(&lang_dir, Some("en")).expect("Failed to load languages");

    assert_eq!(idiomas.len(), 23, "Must have exactly 23 languages");
    for item in idiomas.list() {
        assert!(
            !item.name.trim().is_empty(),
            "Language {} has empty name",
            item.code
        );
    }
}

#[test]
fn test_no_language_file_has_emoji() {
    let root = get_project_root();
    let lang_dir = root.join("languages");
    let emoji_re = Regex::new(r"\p{Extended_Pictographic}").unwrap();

    let entries = fs::read_dir(&lang_dir).unwrap();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let content = fs::read_to_string(&path).unwrap();
            assert!(
                !emoji_re.is_match(&content),
                "File {:?} contains emoji",
                path.file_name()
            );
        }
    }
}

#[test]
fn test_used_keys_exist_in_en_and_translations_have_valid_placeholders() {
    let root = get_project_root();
    let mut files = Vec::new();
    collect_source_files(&root.join("src"), &["js"], &mut files);
    collect_source_files(&root.join("rust").join("crates"), &["rs"], &mut files);

    let key_pattern = Regex::new(
        r#"['"]((?:commands|buttons|buttonhandler|modalhandler|musicplayer|musicmanager|errors|ui|youtube)\.[\w.]+)['"]"#,
    )
    .unwrap();

    let mut used_keys = HashSet::new();
    for file in files {
        if let Ok(content) = fs::read_to_string(&file) {
            for cap in key_pattern.captures_iter(&content) {
                let k = &cap[1];
                if !k.ends_with(".com") && !k.ends_with(".hash") {
                    used_keys.insert(k.to_string());
                }
            }
        }
    }

    assert!(
        used_keys.len() > 50,
        "Found only {} used keys",
        used_keys.len()
    );

    let lang_dir = root.join("languages");
    let idiomas = Idiomas::cargar(&lang_dir, Some("en")).unwrap();
    let en_data = idiomas
        .raw_language_data("en")
        .expect("en.json must be loaded");

    let mut missing_in_en = Vec::new();
    for key in &used_keys {
        if lookup(en_data, key).is_none() && !key.ends_with(".description") {
            missing_in_en.push(key.clone());
        }
    }
    assert!(
        missing_in_en.is_empty(),
        "Keys missing in en.json: {:?}",
        missing_in_en
    );

    let mut placeholder_errors = Vec::new();
    for (code, lang_val) in idiomas.iter_languages() {
        for key in &used_keys {
            if let Some(text) = lookup(lang_val, key) {
                if let Some(en_text) = lookup(en_data, key) {
                    let allowed = extract_placeholders(en_text);
                    for ph in extract_placeholders(text) {
                        if !allowed.contains(&ph) {
                            placeholder_errors.push(format!("{code}:{key}:{{{ph}}}"));
                        }
                    }
                }
            }
        }
    }
    assert!(
        placeholder_errors.is_empty(),
        "Placeholder errors: {:?}",
        placeholder_errors
    );
}

#[test]
fn test_bot_detection_error_points_to_readme() {
    let root = get_project_root();
    let lang_dir = root.join("languages");
    let idiomas = Idiomas::cargar(&lang_dir, Some("en")).unwrap();
    let en_data = idiomas.raw_language_data("en").unwrap();

    let text = lookup(en_data, "errors.youtube_bot_detection")
        .expect("errors.youtube_bot_detection must exist in en.json");
    assert!(text.contains("COOKIES_FILE"));
    assert!(text.contains("README.md"));

    let readme = fs::read_to_string(root.join("README.md")).expect("README.md must exist");
    assert!(readme.contains("YouTube: Sign in to confirm you're not a bot"));
}
