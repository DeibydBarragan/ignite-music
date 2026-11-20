use crate::vosk::{VoskError, VoskModel, VoskRecognizer, VoskWrapper};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub trait VoskBackend: Send + Sync {
    fn load_model(&self, path: &Path) -> Result<usize, VoskError>;
    fn free_model(&self, model: usize);
    fn has_word(&self, model: usize, word: &str) -> bool;
    fn new_recognizer(&self, model: usize, sample_rate: f32) -> Result<usize, VoskError>;
    fn free_recognizer(&self, rec: usize);
    fn accept_waveform(&self, rec: usize, pcm: &[u8]) -> i32;
    fn final_result(&self, rec: usize) -> String;
    fn reset(&self, rec: usize);
    fn set_words(&self, rec: usize, enabled: bool);
}

impl VoskBackend for VoskWrapper {
    fn load_model(&self, path: &Path) -> Result<usize, VoskError> {
        self.create_model(path).map(|p| p as usize)
    }
    fn free_model(&self, model: usize) {
        unsafe { self.free_model(model as *mut VoskModel) };
    }
    fn has_word(&self, model: usize, word: &str) -> bool {
        unsafe { self.has_word(model as *mut VoskModel, word) }
    }
    fn new_recognizer(&self, model: usize, sample_rate: f32) -> Result<usize, VoskError> {
        unsafe { self.create_recognizer(model as *mut VoskModel, sample_rate) }.map(|p| p as usize)
    }
    fn free_recognizer(&self, rec: usize) {
        unsafe { self.free_recognizer(rec as *mut VoskRecognizer) };
    }
    fn accept_waveform(&self, rec: usize, pcm: &[u8]) -> i32 {
        unsafe { self.accept_waveform(rec as *mut VoskRecognizer, pcm) }
    }
    fn final_result(&self, rec: usize) -> String {
        unsafe { self.get_final_result(rec as *mut VoskRecognizer) }
    }
    fn reset(&self, rec: usize) {
        unsafe { self.reset(rec as *mut VoskRecognizer) };
    }
    fn set_words(&self, rec: usize, enabled: bool) {
        unsafe { self.set_words(rec as *mut VoskRecognizer, enabled) };
    }
}

struct Modelo {
    backend: Arc<dyn VoskBackend>,
    handle: usize,
}

impl Drop for Modelo {
    fn drop(&mut self) {
        self.backend.free_model(self.handle);
    }
}

struct Reconocedor(usize);

#[derive(Default)]
struct EstadoModelo {
    modelo: Option<Arc<Modelo>>,
    pool: Vec<Reconocedor>,
}

pub struct VoskModelManager {
    estado: Mutex<EstadoModelo>,
    model_path: Option<PathBuf>,
}

pub struct PrestamoReconocedor<'a> {
    gestor: &'a VoskModelManager,
    modelo: Arc<Modelo>,
    reconocedor: Option<Reconocedor>,
}

impl PrestamoReconocedor<'_> {
    pub fn procesar_enunciado(&mut self, pcm: &[u8]) -> String {
        if pcm.is_empty() {
            return String::new();
        }
        let rec = self.reconocedor.as_ref().expect("Reconocedor activo").0;
        self.modelo.backend.accept_waveform(rec, pcm);
        let text = self.modelo.backend.final_result(rec);
        self.modelo.backend.reset(rec);
        text.trim().to_lowercase()
    }
}

impl Drop for PrestamoReconocedor<'_> {
    fn drop(&mut self) {
        let Some(rec) = self.reconocedor.take() else {
            return;
        };
        self.modelo.backend.reset(rec.0);
        let mut estado = self.gestor.estado.lock().unwrap();
        if estado
            .modelo
            .as_ref()
            .is_some_and(|actual| Arc::ptr_eq(actual, &self.modelo))
            && estado.pool.len() < 4
        {
            estado.pool.push(rec);
        } else {
            self.modelo.backend.free_recognizer(rec.0);
        }
    }
}

impl VoskModelManager {
    pub fn initialize(dll_path: &Path, model_dir: &Path) -> Result<Self, VoskError> {
        let wrapper = VoskWrapper::init(dll_path)?;
        Self::initialize_with_backend(Arc::new(wrapper), model_dir)
    }

    pub fn initialize_with_backend(
        backend: Arc<dyn VoskBackend>,
        model_dir: &Path,
    ) -> Result<Self, VoskError> {
        let handle = match backend.load_model(model_dir) {
            Ok(handle) => handle,
            Err(error) => {
                std::thread::sleep(std::time::Duration::from_millis(200));
                backend.load_model(model_dir).map_err(|_| error)?
            }
        };
        Ok(Self {
            estado: Mutex::new(EstadoModelo {
                modelo: Some(Arc::new(Modelo { backend, handle })),
                pool: Vec::with_capacity(4),
            }),
            model_path: Some(model_dir.to_path_buf()),
        })
    }

    pub fn from_backend(_backend: Arc<dyn VoskBackend>) -> Self {
        Self {
            estado: Mutex::default(),
            model_path: None,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.estado.lock().unwrap().modelo.is_some()
    }

    pub fn model_path(&self) -> Option<PathBuf> {
        self.model_path.clone()
    }

    pub fn has_word(&self, word: &str) -> bool {
        let modelo = self.estado.lock().unwrap().modelo.clone();
        modelo.is_some_and(|m| m.backend.has_word(m.handle, word))
    }

    pub fn adquirir(&self) -> Option<PrestamoReconocedor<'_>> {
        let (modelo, rec) = {
            let mut estado = self.estado.lock().unwrap();
            (estado.modelo.clone()?, estado.pool.pop())
        };
        let rec = match rec {
            Some(rec) => rec,
            None => Reconocedor(
                modelo
                    .backend
                    .new_recognizer(modelo.handle, 16_000.0)
                    .ok()?,
            ),
        };
        Some(PrestamoReconocedor {
            gestor: self,
            modelo,
            reconocedor: Some(rec),
        })
    }

    pub fn destroy(&self) {
        let EstadoModelo { modelo, pool } = std::mem::take(&mut *self.estado.lock().unwrap());
        if let Some(modelo) = modelo {
            for rec in pool {
                modelo.backend.free_recognizer(rec.0);
            }
        }
    }
}

impl Drop for VoskModelManager {
    fn drop(&mut self) {
        self.destroy();
    }
}

#[cfg(test)]
mod pruebas;
