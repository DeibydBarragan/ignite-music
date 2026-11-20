use sha1::{Digest, Sha1};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct YtDlpSettings {
    pub path: Option<String>,
    pub concurrency: usize,
    pub limiter: crate::Limitador,
    pub timeout_ms: u64,
    pub cookies_file: Option<String>,
    pub cookies_from_browser: Option<String>,
    pub po_token: Option<String>,
    pub extractor_args: Option<String>,
    pub js_runtime: Option<String>,
    pub bin_dir: PathBuf,
}

impl Default for YtDlpSettings {
    fn default() -> Self {
        Self {
            path: None,
            concurrency: 4,
            limiter: crate::Limitador::new(4),
            timeout_ms: 30_000,
            cookies_file: None,
            cookies_from_browser: None,
            po_token: None,
            extractor_args: None,
            js_runtime: None,
            bin_dir: PathBuf::from("bin"),
        }
    }
}

pub fn resolve_binary(settings: &YtDlpSettings) -> String {
    if let Some(p) = &settings.path {
        if !p.is_empty() {
            return p.clone();
        }
    }
    let binary_name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let local = settings.bin_dir.join("yt-dlp-dist").join(binary_name);
    if local.exists() {
        local.to_string_lossy().to_string()
    } else {
        binary_name.to_string()
    }
}

pub fn proxy_for(key: Option<&str>, proxies: &[String]) -> Option<String> {
    if proxies.is_empty() {
        return None;
    }
    if proxies.len() == 1 || key.is_none() {
        return Some(proxies[0].clone());
    }
    let k = key.unwrap();
    let mut hasher = Sha1::new();
    hasher.update(k.as_bytes());
    let digest = hasher.finalize();
    let val = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    let index = (val as usize) % proxies.len();
    Some(proxies[index].clone())
}

pub fn base_args(
    settings: &YtDlpSettings,
    proxy: Option<&str>,
    po_token: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "--ignore-config".to_string(),
        "--no-warnings".to_string(),
        "--no-progress".to_string(),
        "--socket-timeout".to_string(),
        "30".to_string(),
        "--retries".to_string(),
        "10".to_string(),
        "--fragment-retries".to_string(),
        "10".to_string(),
        "--retry-sleep".to_string(),
        "fragment:1".to_string(),
        "--buffer-size".to_string(),
        "16M".to_string(),
        "--force-ipv4".to_string(),
    ];

    if let Some(js) = &settings.js_runtime {
        if !js.is_empty() {
            args.push("--js-runtimes".to_string());
            args.push(js.clone());
        }
    }

    if let Some(cf) = &settings.cookies_file {
        args.push("--cookies".to_string());
        args.push(cf.clone());
    } else if let Some(cb) = &settings.cookies_from_browser {
        args.push("--cookies-from-browser".to_string());
        args.push(cb.clone());
    }

    let mut extractor_args = Vec::new();
    let token = po_token.or(settings.po_token.as_deref());
    if let Some(t) = token {
        extractor_args.push(format!(
            "youtube:po_token=web.gvs+{t};player_client=web,default"
        ));
    }
    if let Some(ea) = &settings.extractor_args {
        extractor_args.push(ea.clone());
    }
    for val in extractor_args {
        args.push("--extractor-args".to_string());
        args.push(val);
    }

    if let Some(p) = proxy {
        args.push("--proxy".to_string());
        args.push(p.to_string());
    }

    args
}
