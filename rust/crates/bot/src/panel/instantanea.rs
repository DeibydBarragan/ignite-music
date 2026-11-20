use medios::Cancion;
use player::{Cola, ModoRepeticion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantaneaCola {
    pub cancion: Cancion,
    pub pausada: bool,
    pub tiempo_actual: u64,
    pub volumen: u8,
    pub modo_repeticion: u8,
    pub autoplay: bool,
    pub filtro: Option<String>,
    pub proximas: Vec<Cancion>,
    pub duracion_proximas: u64,
    pub requester_id: Option<String>,
    pub requester_es_bot: bool,
    pub proxima_requester_id: Option<String>,
    pub proxima_requester_es_bot: bool,
}

impl InstantaneaCola {
    pub fn desde_cola(cola: &Cola) -> Option<Self> {
        if cola.detenida || cola.canciones.is_empty() {
            return None;
        }
        let actual = cola.cancion_actual()?;
        let proximas: Vec<Cancion> = cola
            .canciones
            .iter()
            .skip(1)
            .map(|e| e.cancion.clone())
            .collect();
        let duracion_proximas = proximas.iter().map(|c| c.duracion).sum();
        let modo_repeticion = match cola.modo_repeticion {
            ModoRepeticion::Apagado => 0,
            ModoRepeticion::Cancion => 1,
            ModoRepeticion::Cola => 2,
        };
        let filtro = cola.filtros.first().cloned();
        let (proxima_req_id, proxima_req_bot) = cola
            .canciones
            .get(1)
            .map(|e| (e.requester_id.clone(), false))
            .unwrap_or((None, false));

        Some(Self {
            cancion: actual.cancion.clone(),
            pausada: cola.pausada,
            tiempo_actual: cola.posicion_segundos,
            volumen: cola.volumen,
            modo_repeticion,
            autoplay: cola.autoplay,
            filtro,
            proximas,
            duracion_proximas,
            requester_id: actual.requester_id.clone(),
            requester_es_bot: false,
            proxima_requester_id: proxima_req_id,
            proxima_requester_es_bot: proxima_req_bot,
        })
    }
}
