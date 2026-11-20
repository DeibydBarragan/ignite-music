use crate::vosk::{VoskError, VoskModel, VoskRecognizer, VoskWrapper};
use std::{path::Path, sync::Arc};

pub(super) struct ModeloGramatical {
    wrapper: Arc<VoskWrapper>,
    handle: usize,
}

impl ModeloGramatical {
    pub fn cargar(wrapper: Arc<VoskWrapper>, path: &Path) -> Result<Self, VoskError> {
        let handle = wrapper.create_model(path)? as usize;
        Ok(Self { wrapper, handle })
    }

    pub fn has_word(&self, word: &str) -> bool {
        unsafe { self.wrapper.has_word(self.handle as *mut VoskModel, word) }
    }

    pub fn reconocer(&self, pcm: &[u8], gramatica: &str) -> Option<String> {
        let handle = unsafe {
            self.wrapper
                .create_recognizer_grm(self.handle as *mut VoskModel, 16_000.0, gramatica)
        }
        .ok()?;
        let rec = ReconocedorGramatical {
            modelo: self,
            handle,
        };
        Some(unsafe {
            self.wrapper.set_words(rec.handle, true);
            self.wrapper.accept_waveform(rec.handle, pcm);
            self.wrapper.get_final_result_raw(rec.handle)
        })
    }
}

impl Drop for ModeloGramatical {
    fn drop(&mut self) {
        unsafe { self.wrapper.free_model(self.handle as *mut VoskModel) };
    }
}

struct ReconocedorGramatical<'a> {
    modelo: &'a ModeloGramatical,
    handle: *mut VoskRecognizer,
}

impl Drop for ReconocedorGramatical<'_> {
    fn drop(&mut self) {
        unsafe { self.modelo.wrapper.free_recognizer(self.handle) };
    }
}
