//! Gestor del modo desarrollador (dev-mode) persistente en disco (Spec S-27).

use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

pub struct GestorDevMode {
    usuarios: RwLock<HashSet<String>>,
    ruta_archivo: PathBuf,
}

impl GestorDevMode {
    pub fn nuevo(ruta_archivo: PathBuf) -> Self {
        let gestor = Self {
            usuarios: RwLock::new(HashSet::new()),
            ruta_archivo,
        };
        gestor.cargar();
        gestor
    }

    pub fn es_dev_mode(&self, user_id: &str) -> bool {
        self.usuarios.read().unwrap().contains(user_id)
    }

    pub fn es_dev_user(&self, user_id: &str, dev_user_ids: &[String]) -> bool {
        dev_user_ids.iter().any(|id| id == user_id) || self.es_dev_mode(user_id)
    }

    pub fn establecer_dev_mode(&self, user_id: &str, habilitado: bool) -> bool {
        let uid = user_id.trim().to_string();
        let mut set = self.usuarios.write().unwrap();
        if habilitado {
            set.insert(uid);
        } else {
            set.remove(&uid);
        }
        drop(set);
        self.persistir();
        self.es_dev_mode(user_id)
    }

    fn cargar(&self) {
        if let Ok(contenido) = fs::read_to_string(&self.ruta_archivo) {
            if let Ok(v) = serde_json::from_str::<Value>(&contenido) {
                if let Some(arr) = v.get("users").and_then(|u| u.as_array()) {
                    let mut set = self.usuarios.write().unwrap();
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            set.insert(s.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    fn persistir(&self) {
        let set = self.usuarios.read().unwrap();
        let lista: Vec<&String> = set.iter().collect();
        let ahora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let json = json!({
            "users": lista,
            "updatedAt": format!("{ahora}")
        });
        if let Some(dir) = self.ruta_archivo.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&self.ruta_archivo, json.to_string());
    }
}
