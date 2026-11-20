use std::fs;
use std::path::PathBuf;
use voice::alucinacion::{es_alucinacion, SegmentoWhisper};

#[derive(Debug, serde::Deserialize)]
struct CasoAlucinacion {
    texto: String,
    #[serde(rename = "etiquetaSegmentos")]
    _etiqueta_segmentos: String,
    segmentos: Vec<SegmentoWhisper>,
    #[serde(rename = "esAlucinacion")]
    es_alucinacion: bool,
}

#[test]
fn test_alucinacion_paridad_con_node() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let json_path = manifest_dir
        .join("tests")
        .join("data")
        .join("alucinacion-casos.json");
    let content = fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("Failed reading {}: {:?}", json_path.display(), e));
    let casos: Vec<CasoAlucinacion> =
        serde_json::from_str(&content).expect("Invalid alucinacion-casos.json");

    assert!(!casos.is_empty());
    for caso in casos {
        let resultado = es_alucinacion(&caso.texto, &caso.segmentos);
        assert_eq!(
            resultado, caso.es_alucinacion,
            "Mismatch on text='{}', segments={:?}. Expected {}, got {}",
            caso.texto, caso.segmentos, caso.es_alucinacion, resultado
        );
    }
}
