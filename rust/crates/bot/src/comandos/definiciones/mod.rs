//! Definiciones de comandos de barra para Discord (Spec S-22).

pub mod ajustes;
pub mod cola;
pub mod reproduccion;
pub(crate) mod reproduccion_a;
pub(crate) mod reproduccion_b;

use nucleo::Idiomas;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EleccionOpcion {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OpcionComando {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocomplete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<EleccionOpcion>>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_value: Option<i64>,
    pub name: String,
    pub required: bool,
    #[serde(rename = "type")]
    pub tipo: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CuerpoComando {
    pub contexts: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_member_permissions: Option<String>,
    pub description: String,
    pub description_localizations: BTreeMap<String, String>,
    pub name: String,
    pub options: Vec<OpcionComando>,
    #[serde(rename = "type")]
    pub tipo: u8,
}

pub fn comando_base(name: &str, description: &str, idiomas: &Idiomas) -> CuerpoComando {
    let key = format!("commands.{}.description", name);
    let mut description_localizations = BTreeMap::new();
    for (k, v) in idiomas.localizations(&key) {
        description_localizations.insert(k, v);
    }
    CuerpoComando {
        contexts: vec![0],
        default_member_permissions: None,
        description: description.to_string(),
        description_localizations,
        name: name.to_string(),
        options: Vec::new(),
        tipo: 1,
    }
}

pub fn opcion_string(
    name: &str,
    description: &str,
    required: bool,
    max_length: Option<u32>,
    autocomplete: Option<bool>,
    choices: Option<Vec<EleccionOpcion>>,
    description_localizations: Option<BTreeMap<String, String>>,
) -> OpcionComando {
    OpcionComando {
        autocomplete,
        choices,
        description: description.to_string(),
        description_localizations,
        max_length,
        max_value: None,
        min_value: None,
        name: name.to_string(),
        required,
        tipo: 3,
    }
}

pub fn opcion_integer(
    name: &str,
    description: &str,
    required: bool,
    min_value: Option<i64>,
    max_value: Option<i64>,
) -> OpcionComando {
    OpcionComando {
        autocomplete: None,
        choices: None,
        description: description.to_string(),
        description_localizations: None,
        max_length: None,
        max_value,
        min_value,
        name: name.to_string(),
        required,
        tipo: 4,
    }
}

pub fn opcion_boolean(name: &str, description: &str, required: bool) -> OpcionComando {
    OpcionComando {
        autocomplete: None,
        choices: None,
        description: description.to_string(),
        description_localizations: None,
        max_length: None,
        max_value: None,
        min_value: None,
        name: name.to_string(),
        required,
        tipo: 5,
    }
}

pub fn todos_los_cuerpos(idiomas: &Idiomas, max_volume: u32) -> Vec<CuerpoComando> {
    let mut cmds = Vec::with_capacity(25);
    cmds.extend(reproduccion::definiciones(idiomas, max_volume));
    cmds.extend(cola::definiciones(idiomas));
    cmds.extend(ajustes::definiciones(idiomas));
    cmds.sort_by(|a, b| a.name.cmp(&b.name));
    cmds
}
