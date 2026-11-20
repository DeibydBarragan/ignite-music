//! Tipos y utilidades de extracción para el router de interacciones (Spec S-22).

use crate::comandos;
use std::collections::HashMap;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ResultadoEnrutado {
    Comando {
        nombre: String,
        respuesta: Option<String>,
    },
    Autocompletado {
        nombre: String,
        opciones: Vec<comandos::reproducir::EleccionAutocompletado>,
    },
    Componente {
        custom_id: String,
    },
    Modal {
        custom_id: String,
    },
    Desconocido,
    Ignorado,
}

pub fn extraer_opciones(opts: &[CommandDataOption]) -> HashMap<String, serde_json::Value> {
    let mut mapa = HashMap::new();
    for opt in opts {
        let val = match &opt.value {
            CommandOptionValue::String(s) => serde_json::Value::String(s.clone()),
            CommandOptionValue::Integer(i) => serde_json::Value::Number((*i).into()),
            CommandOptionValue::Boolean(b) => serde_json::Value::Bool(*b),
            CommandOptionValue::Number(n) => serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            _ => serde_json::Value::Null,
        };
        mapa.insert(opt.name.clone(), val);
    }
    mapa
}

pub fn extraer_consulta_autocompletado(opts: &[CommandDataOption]) -> String {
    for opt in opts {
        if let CommandOptionValue::Focused(ref s, _) = opt.value {
            return s.clone();
        }
    }
    String::new()
}
