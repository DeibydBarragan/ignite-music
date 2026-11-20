use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

pub enum VoskModel {}
pub enum VoskRecognizer {}

#[derive(Debug, PartialEq, Eq)]
pub enum VoskError {
    Biblioteca(String),
    Modelo(String),
    Reconocedor(String),
}

impl std::fmt::Display for VoskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Biblioteca(msg) => write!(f, "Biblioteca Vosk: {msg}"),
            Self::Modelo(msg) | Self::Reconocedor(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for VoskError {}

fn to_abs(path: &Path) -> PathBuf {
    if path.is_relative() {
        std::env::current_dir().unwrap_or_default().join(path)
    } else {
        path.to_path_buf()
    }
}

unsafe fn sym<T: Copy>(lib: &libloading::Library, name: &[u8]) -> Result<T, VoskError> {
    lib.get(name)
        .map(|s| *s)
        .map_err(|e| VoskError::Biblioteca(e.to_string()))
}

fn check_rec(rec: *mut VoskRecognizer) -> Result<*mut VoskRecognizer, VoskError> {
    (!rec.is_null())
        .then_some(rec)
        .ok_or_else(|| VoskError::Reconocedor("Failed to create VoskRecognizer".into()))
}

type ModelNewFn = unsafe extern "C" fn(*const c_char) -> *mut VoskModel;
type RecNewFn = unsafe extern "C" fn(*mut VoskModel, f32) -> *mut VoskRecognizer;
type RecGrmFn = unsafe extern "C" fn(*mut VoskModel, f32, *const c_char) -> *mut VoskRecognizer;
type RecWaveFn = unsafe extern "C" fn(*mut VoskRecognizer, *const u8, c_int) -> c_int;

pub struct VoskApi {
    _lib: libloading::Library,
    _set_log_level: unsafe extern "C" fn(log_level: c_int),
    model_new: ModelNewFn,
    model_free: unsafe extern "C" fn(model: *mut VoskModel),
    model_find_word: unsafe extern "C" fn(model: *mut VoskModel, word: *const c_char) -> c_int,
    recognizer_new: RecNewFn,
    recognizer_new_grm: RecGrmFn,
    recognizer_free: unsafe extern "C" fn(recognizer: *mut VoskRecognizer),
    recognizer_accept_waveform: RecWaveFn,
    recognizer_final_result: unsafe extern "C" fn(recognizer: *mut VoskRecognizer) -> *const c_char,
    recognizer_reset: unsafe extern "C" fn(recognizer: *mut VoskRecognizer),
    recognizer_set_words: unsafe extern "C" fn(recognizer: *mut VoskRecognizer, words: c_int),
}

unsafe impl Send for VoskApi {}
unsafe impl Sync for VoskApi {}

impl VoskApi {
    pub fn load(path: &Path) -> Result<Self, VoskError> {
        let abs = to_abs(path);
        #[cfg(windows)]
        let lib = unsafe {
            use libloading::os::windows::Library as WinLib;
            WinLib::load_with_flags(&abs, 0x0000_0008)
                .map(libloading::Library::from)
                .map_err(|e| {
                    VoskError::Biblioteca(format!("No se pudo cargar {}: {e}", abs.display()))
                })?
        };
        #[cfg(not(windows))]
        let lib = unsafe {
            libloading::Library::new(&abs).map_err(|e| {
                VoskError::Biblioteca(format!("No se pudo cargar {}: {e}", abs.display()))
            })?
        };

        unsafe {
            let set_log_level: unsafe extern "C" fn(c_int) = sym(&lib, b"vosk_set_log_level\0")?;
            (set_log_level)(-1);
            Ok(Self {
                _set_log_level: set_log_level,
                model_new: sym(&lib, b"vosk_model_new\0")?,
                model_free: sym(&lib, b"vosk_model_free\0")?,
                model_find_word: sym(&lib, b"vosk_model_find_word\0")?,
                recognizer_new: sym(&lib, b"vosk_recognizer_new\0")?,
                recognizer_new_grm: sym(&lib, b"vosk_recognizer_new_grm\0")?,
                recognizer_free: sym(&lib, b"vosk_recognizer_free\0")?,
                recognizer_accept_waveform: sym(&lib, b"vosk_recognizer_accept_waveform\0")?,
                recognizer_final_result: sym(&lib, b"vosk_recognizer_final_result\0")?,
                recognizer_reset: sym(&lib, b"vosk_recognizer_reset\0")?,
                recognizer_set_words: sym(&lib, b"vosk_recognizer_set_words\0")?,
                _lib: lib,
            })
        }
    }
}

static INSTANCE: OnceLock<Arc<VoskApi>> = OnceLock::new();
static INIT_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Clone)]
pub struct VoskWrapper {
    api: Arc<VoskApi>,
}

#[allow(clippy::missing_safety_doc)]
impl VoskWrapper {
    pub fn init(path: &Path) -> Result<Self, VoskError> {
        if let Some(api) = INSTANCE.get() {
            return Ok(Self {
                api: Arc::clone(api),
            });
        }
        let _guard = INIT_MUTEX.lock().unwrap();
        if let Some(api) = INSTANCE.get() {
            return Ok(Self {
                api: Arc::clone(api),
            });
        }
        let api = Arc::new(VoskApi::load(path)?);
        let _ = INSTANCE.set(Arc::clone(&api));
        Ok(Self { api })
    }

    pub fn create_model(&self, path: &Path) -> Result<*mut VoskModel, VoskError> {
        let abs = to_abs(path);
        let c_path = CString::new(abs.to_string_lossy().as_bytes())
            .map_err(|e| VoskError::Modelo(e.to_string()))?;
        let model = unsafe { (self.api.model_new)(c_path.as_ptr()) };
        if model.is_null() {
            return Err(VoskError::Modelo(format!(
                "Failed to load Vosk model from \"{}\"",
                path.display()
            )));
        }
        Ok(model)
    }

    pub unsafe fn free_model(&self, model: *mut VoskModel) {
        if !model.is_null() {
            (self.api.model_free)(model);
        }
    }

    pub unsafe fn has_word(&self, model: *mut VoskModel, word: &str) -> bool {
        let clean = word.trim().to_lowercase();
        if clean.is_empty() || model.is_null() {
            return false;
        }
        let Ok(c_word) = CString::new(clean) else {
            return false;
        };
        (self.api.model_find_word)(model, c_word.as_ptr()) >= 0
    }

    pub unsafe fn create_recognizer(
        &self,
        model: *mut VoskModel,
        sample_rate: f32,
    ) -> Result<*mut VoskRecognizer, VoskError> {
        if model.is_null() {
            return Err(VoskError::Reconocedor(
                "Failed to create VoskRecognizer".into(),
            ));
        }
        check_rec((self.api.recognizer_new)(model, sample_rate))
    }

    pub unsafe fn create_recognizer_grm(
        &self,
        model: *mut VoskModel,
        sample_rate: f32,
        grammar: &str,
    ) -> Result<*mut VoskRecognizer, VoskError> {
        if model.is_null() {
            return Err(VoskError::Reconocedor(
                "Failed to create VoskRecognizer".into(),
            ));
        }
        let c_grm = CString::new(grammar).map_err(|e| VoskError::Reconocedor(e.to_string()))?;
        check_rec((self.api.recognizer_new_grm)(
            model,
            sample_rate,
            c_grm.as_ptr(),
        ))
    }

    pub unsafe fn free_recognizer(&self, rec: *mut VoskRecognizer) {
        if !rec.is_null() {
            (self.api.recognizer_free)(rec);
        }
    }

    pub unsafe fn accept_waveform(&self, rec: *mut VoskRecognizer, buffer: &[u8]) -> i32 {
        if rec.is_null() || buffer.is_empty() {
            return 0;
        }
        (self.api.recognizer_accept_waveform)(rec, buffer.as_ptr(), buffer.len() as c_int)
    }

    pub unsafe fn get_final_result_raw(&self, rec: *mut VoskRecognizer) -> String {
        if rec.is_null() {
            return String::new();
        }
        let ptr = (self.api.recognizer_final_result)(rec);
        if ptr.is_null() {
            return String::new();
        }
        CStr::from_ptr(ptr).to_str().unwrap_or_default().to_string()
    }

    pub unsafe fn get_final_result(&self, rec: *mut VoskRecognizer) -> String {
        let raw = self.get_final_result_raw(rec);
        #[derive(serde::Deserialize)]
        struct VoskJson {
            #[serde(default)]
            text: String,
        }
        serde_json::from_str::<VoskJson>(&raw)
            .map(|j| j.text)
            .unwrap_or_default()
    }

    pub unsafe fn reset(&self, rec: *mut VoskRecognizer) {
        if !rec.is_null() {
            (self.api.recognizer_reset)(rec);
        }
    }

    pub unsafe fn set_words(&self, rec: *mut VoskRecognizer, enabled: bool) {
        if !rec.is_null() {
            (self.api.recognizer_set_words)(rec, if enabled { 1 } else { 0 });
        }
    }
}
