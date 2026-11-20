//! Pruebas unitarias para el anuncio de la URL pública a Supabase (Spec S-29).

use bot::api::anuncio::{calcular_url_publica, Anunciador, ClienteSupabase};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

struct MockSupabase {
    llamadas: AtomicUsize,
    fallar_primera: AtomicBool,
    urls_recibidas: Mutex<Vec<String>>,
}

impl MockSupabase {
    fn nuevo(fallar_primera: bool) -> Self {
        Self {
            llamadas: AtomicUsize::new(0),
            fallar_primera: AtomicBool::new(fallar_primera),
            urls_recibidas: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl ClienteSupabase for MockSupabase {
    async fn upsert_bot_status(&self, api_url: &str) -> Result<(), String> {
        let n = self.llamadas.fetch_add(1, Ordering::SeqCst);
        self.urls_recibidas.lock().await.push(api_url.to_string());
        if n == 0 && self.fallar_primera.load(Ordering::SeqCst) {
            Err("Error simulado de red en Supabase".to_string())
        } else {
            Ok(())
        }
    }
}

#[test]
fn test_calcular_url_publica_variantes() {
    // 1. Con túnel activo: gana sobre PUBLIC_API_URL y recorta barras finales
    let url = calcular_url_publica(
        Some("https://alpha-beta.trycloudflare.com///"),
        "https://ignore-me.com/",
    );
    assert_eq!(url, "https://alpha-beta.trycloudflare.com");

    // 2. Sin túnel, con PUBLIC_API_URL válida con barras finales
    let url = calcular_url_publica(None, "https://api.ignite.com/bot//");
    assert_eq!(url, "https://api.ignite.com/bot");

    // 3. Con esquema http:// no seguro
    let url = calcular_url_publica(None, "http://localhost:3001/");
    assert_eq!(url, "http://localhost:3001");

    // 4. Esquema inválido (ftp, etc.) queda vacía
    let url = calcular_url_publica(None, "ftp://ftp.example.com/");
    assert_eq!(url, "");

    // 5. Cadena que no es URL queda vacía
    let url = calcular_url_publica(None, "localhost:3001");
    assert_eq!(url, "");

    // 6. Cadena vacía
    let url = calcular_url_publica(None, "   ");
    assert_eq!(url, "");
}

#[tokio::test]
async fn test_anuncio_sin_url_o_sin_supabase_no_hace_nada() {
    let mock = Arc::new(MockSupabase::nuevo(false));

    // Sin URL pública configurada
    let anunciador_sin_url = Anunciador::nuevo(Some(mock.clone()), None, "".to_string());
    assert_eq!(anunciador_sin_url.url_publica(), "");
    let iniciado = anunciador_sin_url.iniciar(Duration::from_millis(50));
    assert!(!iniciado, "No debe iniciar sin URL pública");
    assert_eq!(anunciador_sin_url.beat().await, Ok(()));
    assert_eq!(mock.llamadas.load(Ordering::SeqCst), 0);

    // Sin cliente Supabase
    let anunciador_sin_sb = Anunciador::nuevo(None, None, "https://api.ejemplo.com".to_string());
    assert_eq!(anunciador_sin_sb.url_publica(), "https://api.ejemplo.com");
    let iniciado = anunciador_sin_sb.iniciar(Duration::from_millis(50));
    assert!(!iniciado, "No debe iniciar sin cliente Supabase");
    assert_eq!(anunciador_sin_sb.beat().await, Ok(()));
}

#[tokio::test]
async fn test_anuncio_latido_y_tolerancia_a_errores() {
    // Configurado para fallar en la primera llamada
    let mock = Arc::new(MockSupabase::nuevo(true));
    let anunciador = Anunciador::nuevo(
        Some(mock.clone()),
        None,
        "https://api.ignite.com".to_string(),
    );

    // Iniciar con intervalo rápido para reloj simulado de pruebas (20ms)
    let iniciado = anunciador.iniciar(Duration::from_millis(20));
    assert!(iniciado);

    // Esperar a que se ejecute la llamada inicial y varios latidos
    tokio::time::sleep(Duration::from_millis(70)).await;

    // Detener el anunciador
    anunciador.detener().await;

    // Se realizaron múltiples llamadas a pesar del error en la llamada inicial
    let total = mock.llamadas.load(Ordering::SeqCst);
    assert!(
        total >= 2,
        "Debe haber ejecutado al menos 2 llamadas (latido continuo tras error), total: {total}"
    );

    let urls = mock.urls_recibidas.lock().await;
    for u in urls.iter() {
        assert_eq!(u, "https://api.ignite.com");
    }
}
