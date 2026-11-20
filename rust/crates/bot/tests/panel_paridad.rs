use bot::panel::InstantaneaCola;
use bot::ui::panel::vista_reproductor;
use medios::Cancion;
use nucleo::idioma::Idiomas;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn fake_song(n: u64, name: Option<String>, live: bool, dur: u64) -> Cancion {
    Cancion {
        fuente: "youtube".into(),
        id: format!("vid{:08}", n),
        nombre: name.unwrap_or_else(|| format!("Song {n}")),
        artista: format!("Artist {n}"),
        url: format!("https://www.youtube.com/watch?v=vid{:08}", n),
        stream_url: None,
        es_directo: live,
        duracion: dur,
        miniatura: Some(format!("https://i.ytimg.com/vi/vid{n}/hqdefault.jpg")),
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    }
}

fn base_snap() -> InstantaneaCola {
    let s = fake_song(1, None, false, 181);
    let up: Vec<Cancion> = (2..=30)
        .map(|i| fake_song(i, None, false, 180 + i))
        .collect();
    InstantaneaCola {
        cancion: s,
        pausada: false,
        tiempo_actual: 42,
        volumen: 80,
        modo_repeticion: 0,
        autoplay: false,
        filtro: None,
        proximas: up,
        duracion_proximas: 5000,
        requester_id: None,
        requester_es_bot: false,
        proxima_requester_id: None,
        proxima_requester_es_bot: false,
    }
}

fn cargar_idiomas() -> Arc<Idiomas> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../languages");
    Arc::new(Idiomas::cargar(&path, Some("en")).unwrap())
}

fn normalize(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            map.retain(|k, v| {
                !v.is_null() && !(k == "disabled" && v == &serde_json::Value::Bool(false))
            });
            for v in map.values_mut() {
                normalize(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                normalize(v);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn test_paridad_con_node() {
    let idiomas = cargar_idiomas();
    let fix_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/panel-node.json");
    let json_str = std::fs::read_to_string(&fix_path).expect("panel-node.json exists");
    let fixture: HashMap<String, serde_json::Value> = serde_json::from_str(&json_str).unwrap();

    let mut casos = HashMap::new();
    casos.insert("reproduciendo", base_snap());
    let mut p2 = base_snap();
    p2.pausada = true;
    p2.modo_repeticion = 2;
    p2.autoplay = true;
    casos.insert("en_pausa_bucle_cola_autoplay", p2);
    let mut p3 = base_snap();
    p3.cancion = fake_song(98, None, true, 278);
    casos.insert("en_directo", p3);
    let mut p4 = base_snap();
    p4.cancion = fake_song(96, None, false, 0);
    casos.insert("duracion_cero", p4);
    let mut p5 = base_snap();
    p5.cancion = fake_song(99, Some("x".repeat(500)), false, 279);
    p5.cancion.artista = "y".repeat(300);
    casos.insert("nombre_500_caracteres", p5);
    let mut p6 = base_snap();
    p6.proximas.clear();
    p6.duracion_proximas = 0;
    casos.insert("sin_proximas", p6);
    let mut p7 = base_snap();
    p7.filtro = Some("nightcore".into());
    casos.insert("con_filtro", p7);

    for (k, snap) in casos {
        let rust_payload = vista_reproductor(&idiomas, "en", &snap);
        let exp = &fixture[k];
        let rust_flags = rust_payload.flags.unwrap().bits();
        assert_eq!(
            rust_flags,
            exp["flags"].as_u64().unwrap(),
            "flags mismatch in {k}"
        );
        let mut rust_comp = serde_json::to_value(&rust_payload.components).unwrap();
        let mut exp_comp = exp["components"].clone();
        normalize(&mut rust_comp);
        normalize(&mut exp_comp);
        assert_eq!(rust_comp, exp_comp, "components mismatch in {k}");
    }
}
