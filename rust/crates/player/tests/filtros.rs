use player::{
    crear_comando_ffmpeg, es_filtro_valido, ffmpeg_args, ffmpeg_pipe_args, fijar_filtro_cola,
    filtro_etiqueta, filtro_ffmpeg_arg, resolver_ffmpeg, Cola, FILTROS,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize)]
struct FilterCase {
    name: String,
    label: String,
    ffmpeg_arg: String,
    audio_sha256: Option<String>,
    #[allow(dead_code)]
    audio_bytes: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ParityCases {
    fixture: Option<String>,
    filters: Vec<FilterCase>,
    order: Vec<String>,
}

#[test]
fn test_filtros_paridad_con_node() {
    let json_bytes = include_bytes!("data/filtros-casos.json");
    let parity: ParityCases = serde_json::from_slice(json_bytes).expect("parse json");

    assert_eq!(
        parity.order,
        FILTROS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    );

    for case in parity.filters {
        assert!(
            es_filtro_valido(&case.name),
            "Filtro '{}' debe ser válido",
            case.name
        );
        assert_eq!(
            filtro_etiqueta(&case.name),
            Some(case.label.as_str()),
            "Etiqueta de '{}' no coincide",
            case.name
        );
        assert_eq!(
            filtro_ffmpeg_arg(&case.name),
            Some(case.ffmpeg_arg.as_str()),
            "Argumento ffmpeg de '{}' no coincide",
            case.name
        );
    }

    assert!(!es_filtro_valido("desconocido"));
    assert_eq!(filtro_etiqueta("desconocido"), None);
    assert_eq!(filtro_ffmpeg_arg("desconocido"), None);
}

#[test]
fn test_fijar_filtro_cola_comportamiento() {
    let mut cola = Cola::nueva("guild-1");

    let res1 = fijar_filtro_cola(&mut cola, "bassboost");
    assert!(res1.ok && res1.cambio);
    assert_eq!(res1.etiqueta, Some("Bass Boost"));
    assert_eq!(cola.filtros, vec!["bassboost".to_string()]);

    let res2 = fijar_filtro_cola(&mut cola, "bassboost");
    assert!(res2.ok && !res2.cambio);

    let res3 = fijar_filtro_cola(&mut cola, "nightcore");
    assert!(res3.ok && res3.cambio);
    assert_eq!(res3.etiqueta, Some("Nightcore"));

    let res4 = fijar_filtro_cola(&mut cola, "none");
    assert!(res4.ok && res4.cambio);
    assert!(cola.filtros.is_empty());

    let res5 = fijar_filtro_cola(&mut cola, "");
    assert!(res5.ok && !res5.cambio);

    let res6 = fijar_filtro_cola(&mut cola, "invalido");
    assert!(!res6.ok && !res6.cambio);
    assert_eq!(res6.clave_mensaje, "errors.unknown");
}

#[test]
fn test_ffmpeg_args_construccion() {
    let args1 = ffmpeg_args("https://example.com/audio.opus", None, None, true);
    assert!(args1.contains(&"-reconnect".to_string()));
    assert!(!args1.contains(&"-ss".to_string()));
    assert!(!args1.contains(&"-af".to_string()));
    assert_eq!(
        &args1[args1.len() - 7..],
        &["-f", "s16le", "-ac", "2", "-ar", "48000", "pipe:1"]
    );

    let args2 = ffmpeg_args("test.mp3", Some("bassboost"), Some(45), false);
    assert!(!args2.contains(&"-reconnect".to_string()));
    let ss_idx = args2.iter().position(|a| a == "-ss").expect("ss flag");
    assert_eq!(args2[ss_idx + 1], "45");
    let af_idx = args2.iter().position(|a| a == "-af").expect("af flag");
    assert_eq!(args2[af_idx + 1], "bass=g=10");

    let args3 = ffmpeg_args("track.wav", Some("nightcore"), None, false);
    let af_idx3 = args3.iter().position(|a| a == "-af").expect("af flag");
    assert_eq!(
        args3[af_idx3 + 1],
        "asetrate=48000*1.25,aresample=48000,bass=g=5"
    );
}

#[test]
fn test_ffmpeg_pipe_args_aplica_filtro_seek_y_formato_detectable() {
    let args = ffmpeg_pipe_args(Some("bassboost"), Some(42));
    let input = args.iter().position(|arg| arg == "pipe:0").unwrap();
    let seek = args.iter().position(|arg| arg == "-ss").unwrap();
    let filter = args.iter().position(|arg| arg == "-af").unwrap();
    let wav = args.iter().position(|arg| arg == "wav").unwrap();
    assert!(seek > input, "seek en pipe debe ocurrir tras el input");
    assert_eq!(args[seek + 1], "42");
    assert_eq!(args[filter + 1], "bass=g=10");
    assert!(wav > filter);
    assert_eq!(args.last().map(String::as_str), Some("pipe:1"));
}

#[test]
fn test_resolver_ffmpeg_precedencia() {
    std::env::set_var("FFMPEG_PATH", "custom/bin/ffmpeg");
    assert_eq!(resolver_ffmpeg(), "custom/bin/ffmpeg");
    std::env::remove_var("FFMPEG_PATH");
}

#[tokio::test]
async fn test_filtros_paridad_audio_byte_a_byte() {
    let json_bytes = include_bytes!("data/filtros-casos.json");
    let parity: ParityCases = serde_json::from_slice(json_bytes).expect("parse json");

    let ffmpeg_bin = resolver_ffmpeg();
    let fixture_name = parity.fixture.as_deref().unwrap_or("ignite-sample.wav");

    let candidates = [
        Path::new("../../../test/fixtures").join(fixture_name),
        Path::new("../../test/fixtures").join(fixture_name),
        Path::new("test/fixtures").join(fixture_name),
    ];
    let fixture_path = candidates.into_iter().find(|p| p.exists());
    let fixture_path = match fixture_path {
        Some(p) => p,
        None => return,
    };

    let check = std::process::Command::new(&ffmpeg_bin)
        .arg("-version")
        .output();
    if !check.map(|o| o.status.success()).unwrap_or(false) {
        // Spec S-18: cargo test pasa sin ffmpeg instalado
        return;
    }

    for case in parity.filters {
        if let Some(expected_hash) = case.audio_sha256 {
            let mut cmd = crear_comando_ffmpeg(
                &ffmpeg_bin,
                &fixture_path.to_string_lossy(),
                Some(&case.name),
                None,
                false,
            );
            let out = cmd.output().await.expect("ejecutar ffmpeg");
            assert!(
                out.status.success(),
                "ffmpeg falló para filtro {}: {}",
                case.name,
                String::from_utf8_lossy(&out.stderr)
            );

            let mut hasher = Sha256::new();
            hasher.update(&out.stdout);
            let actual_hash = format!("{:x}", hasher.finalize());

            assert_eq!(
                actual_hash, expected_hash,
                "Hash de audio para '{}' no coincide con Node",
                case.name
            );

            let mut runtime = tokio::process::Command::new(&ffmpeg_bin)
                .args(ffmpeg_pipe_args(Some(&case.name), None))
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("iniciar FFmpeg con entrada de tubería");
            runtime
                .stdin
                .take()
                .expect("stdin ffmpeg")
                .write_all(&std::fs::read(&fixture_path).expect("leer WAV de prueba"))
                .await
                .expect("escribir entrada a FFmpeg");
            let output = runtime.wait_with_output().await.expect("esperar FFmpeg");
            assert!(
                output.status.success(),
                "FFmpeg de reproducción falló para {}: {}",
                case.name,
                String::from_utf8_lossy(&output.stderr)
            );
            let pcm = wav_data(&output.stdout);
            let mut hasher = Sha256::new();
            hasher.update(pcm);
            assert_eq!(
                format!("{:x}", hasher.finalize()),
                expected_hash,
                "PCM del pipeline de reproducción no coincide con Node para {}",
                case.name
            );
        }
    }
}

fn wav_data(wav: &[u8]) -> &[u8] {
    let mut offset = 12;
    while offset + 8 <= wav.len() {
        let id = &wav[offset..offset + 4];
        let len = u32::from_le_bytes(wav[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        if id == b"data" {
            return &wav[start..];
        }
        offset = start + len + (len % 2);
    }
    panic!("FFmpeg no produjo un chunk WAV data");
}
