use nucleo::formato::{
    barra_progreso, chunk, enlace, escapar_markdown, etiqueta_enlace, formatear_duracion,
    formatear_uptime, parse_time, truncar, url_segura,
};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct DurCase {
    input: i64,
    output: String,
}

#[derive(Deserialize)]
struct ParseTimeCase {
    input: String,
    output: Option<u64>,
}

#[derive(Deserialize)]
struct BarCase {
    current: u64,
    total: u64,
    size: usize,
    output: String,
}

#[derive(Deserialize)]
struct TruncCase {
    text: String,
    max: usize,
    output: String,
}

#[derive(Deserialize)]
struct EscCase {
    text: String,
    output: String,
}

#[derive(Deserialize)]
struct SafeUrlCase {
    url: String,
    output: Option<String>,
}

#[derive(Deserialize)]
struct LinkLabelCase {
    title: Option<String>,
    max: usize,
    output: String,
}

#[derive(Deserialize)]
struct LinkCase {
    title: Option<String>,
    url: String,
    max: usize,
    output: String,
}

#[derive(Deserialize)]
struct UptimeCase {
    ms: u64,
    output: String,
}

#[derive(Deserialize)]
struct ChunkCase {
    items: Vec<i64>,
    size: usize,
    output: Vec<Vec<i64>>,
}

#[derive(Deserialize)]
struct Fixture {
    #[serde(rename = "formatDuration")]
    format_duration: Vec<DurCase>,
    #[serde(rename = "parseTime")]
    parse_time: Vec<ParseTimeCase>,
    #[serde(rename = "progressBar")]
    progress_bar: Vec<BarCase>,
    truncate: Vec<TruncCase>,
    #[serde(rename = "escapeMarkdown")]
    escape_markdown: Vec<EscCase>,
    #[serde(rename = "safeUrl")]
    safe_url: Vec<SafeUrlCase>,
    #[serde(rename = "linkLabel")]
    link_label: Vec<LinkLabelCase>,
    link: Vec<LinkCase>,
    #[serde(rename = "formatUptime")]
    format_uptime: Vec<UptimeCase>,
    chunk: Vec<ChunkCase>,
}

#[test]
fn test_paridad_formato_con_node() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formato-node.json");
    let content = std::fs::read_to_string(&path).expect("formato-node.json exists");
    let fixture: Fixture = serde_json::from_str(&content).expect("parse JSON");

    for c in fixture.format_duration {
        let sec = if c.input < 0 { 0 } else { c.input as u64 };
        assert_eq!(formatear_duracion(sec), c.output);
    }

    for c in fixture.parse_time {
        assert_eq!(
            parse_time(&c.input),
            c.output,
            "parse_time for '{}'",
            c.input
        );
    }

    for c in fixture.progress_bar {
        assert_eq!(
            barra_progreso(c.current, c.total, c.size),
            c.output,
            "bar for {}/{}",
            c.current,
            c.total
        );
    }

    for c in fixture.truncate {
        assert_eq!(truncar(&c.text, c.max), c.output);
    }

    for c in fixture.escape_markdown {
        assert_eq!(escapar_markdown(&c.text), c.output);
    }

    for c in fixture.safe_url {
        assert_eq!(url_segura(&c.url), c.output);
    }

    for c in fixture.link_label {
        let t = c.title.as_deref().unwrap_or("");
        assert_eq!(etiqueta_enlace(t, c.max), c.output, "linkLabel for '{t}'");
    }

    for c in fixture.link {
        let t = c.title.as_deref().unwrap_or("");
        assert_eq!(enlace(t, &c.url, c.max), c.output, "link for '{t}'");
    }

    for c in fixture.format_uptime {
        assert_eq!(formatear_uptime(c.ms), c.output, "uptime for {}", c.ms);
    }

    for c in fixture.chunk {
        let res = chunk(&c.items, c.size).unwrap();
        assert_eq!(res, c.output, "chunk for size {}", c.size);
    }

    assert!(chunk::<i32>(&[1, 2], 0).is_err());
}
