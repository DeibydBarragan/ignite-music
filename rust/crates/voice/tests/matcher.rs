use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use voice::matcher::{coincidir, FrasePersonalizada};

#[derive(Debug, serde::Deserialize)]
struct CasoMatcher {
    input: String,
    #[serde(rename = "wakeWord")]
    wake_word: String,
    #[serde(rename = "customBindings")]
    custom_bindings: Option<HashMap<String, Vec<String>>>,
    coincide: bool,
    motivo: Option<String>,
    palabra: Option<String>,
    comando: Option<String>,
    frase: Option<String>,
    consulta: Option<String>,
    #[serde(rename = "faltaConsulta")]
    falta_consulta: Option<bool>,
    relleno: Option<String>,
}

#[test]
fn test_matcher_paridad_con_node() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let json_path = manifest_dir
        .join("tests")
        .join("data")
        .join("matcher-casos.json");
    let content = fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("Failed reading {}: {:?}", json_path.display(), e));
    let casos: Vec<CasoMatcher> =
        serde_json::from_str(&content).expect("Invalid matcher-casos.json");

    assert!(!casos.is_empty());
    for caso in casos {
        let mut custom = Vec::new();
        if let Some(map) = &caso.custom_bindings {
            for (cmd, phrases) in map {
                for p in phrases {
                    custom.push(FrasePersonalizada {
                        comando: cmd.clone(),
                        frase: p.clone(),
                    });
                }
            }
        }
        let custom_ref = if custom.is_empty() {
            None
        } else {
            Some(custom.as_slice())
        };

        let res = coincidir(&caso.input, &caso.wake_word, custom_ref);

        assert_eq!(
            res.coincide, caso.coincide,
            "Mismatch on coincide for input='{}', expected {}, got {}",
            caso.input, caso.coincide, res.coincide
        );
        assert_eq!(
            res.motivo, caso.motivo,
            "Mismatch on motivo for input='{}', expected {:?}, got {:?}",
            caso.input, caso.motivo, res.motivo
        );
        assert_eq!(
            res.palabra, caso.palabra,
            "Mismatch on palabra for input='{}', expected {:?}, got {:?}",
            caso.input, caso.palabra, res.palabra
        );
        assert_eq!(
            res.comando, caso.comando,
            "Mismatch on comando for input='{}', expected {:?}, got {:?}",
            caso.input, caso.comando, res.comando
        );
        assert_eq!(
            res.frase, caso.frase,
            "Mismatch on frase for input='{}', expected {:?}, got {:?}",
            caso.input, caso.frase, res.frase
        );
        assert_eq!(
            res.consulta, caso.consulta,
            "Mismatch on consulta for input='{}', expected {:?}, got {:?}",
            caso.input, caso.consulta, res.consulta
        );
        assert_eq!(
            res.falta_consulta, caso.falta_consulta,
            "Mismatch on falta_consulta for input='{}', expected {:?}, got {:?}",
            caso.input, caso.falta_consulta, res.falta_consulta
        );
        assert_eq!(
            res.relleno, caso.relleno,
            "Mismatch on relleno for input='{}', expected {:?}, got {:?}",
            caso.input, caso.relleno, res.relleno
        );
    }
}
