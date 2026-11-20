use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FakeBackend {
    next_id: AtomicUsize,
    resets: AtomicUsize,
    freed: AtomicUsize,
}

impl VoskBackend for FakeBackend {
    fn load_model(&self, _path: &Path) -> Result<usize, VoskError> {
        Ok(999)
    }
    fn free_model(&self, _model: usize) {}
    fn has_word(&self, _m: usize, w: &str) -> bool {
        w == "ignite"
    }
    fn new_recognizer(&self, _m: usize, _r: f32) -> Result<usize, VoskError> {
        Ok(self.next_id.fetch_add(1, Ordering::SeqCst))
    }
    fn free_recognizer(&self, _rec: usize) {
        self.freed.fetch_add(1, Ordering::SeqCst);
    }
    fn accept_waveform(&self, _r: usize, _pcm: &[u8]) -> i32 {
        1
    }
    fn final_result(&self, _r: usize) -> String {
        "{\"text\":\"TEST RESULT\"}".to_string()
    }
    fn reset(&self, _rec: usize) {
        self.resets.fetch_add(1, Ordering::SeqCst);
    }
    fn set_words(&self, _rec: usize, _en: bool) {}
}

#[test]
fn test_pool_sin_modelo_no_devuelve_nada() {
    let fake = Arc::new(FakeBackend {
        next_id: AtomicUsize::new(1),
        resets: AtomicUsize::new(0),
        freed: AtomicUsize::new(0),
    });
    let manager = VoskModelManager::from_backend(fake);
    assert!(!manager.is_ready());
    assert!(manager.adquirir().is_none());
}

#[test]
fn test_buffer_vacio_devuelve_texto_vacio() {
    let fake = Arc::new(FakeBackend {
        next_id: AtomicUsize::new(1),
        resets: AtomicUsize::new(0),
        freed: AtomicUsize::new(0),
    });
    let manager = VoskModelManager::initialize_with_backend(fake, Path::new("fake")).unwrap();
    let mut rec = manager.adquirir().unwrap();
    let res = rec.procesar_enunciado(&[]);
    assert_eq!(res, "");
    drop(rec);
}

#[test]
fn test_pool_reutiliza_resetea_max_4_inactivos_y_libera_quinto() {
    let fake = Arc::new(FakeBackend {
        next_id: AtomicUsize::new(1),
        resets: AtomicUsize::new(0),
        freed: AtomicUsize::new(0),
    });
    let manager =
        VoskModelManager::initialize_with_backend(fake.clone(), Path::new("fake")).unwrap();
    let recs: Vec<_> = (0..5).map(|_| manager.adquirir().unwrap()).collect();
    for rec in recs {
        drop(rec);
    }
    assert_eq!(fake.resets.load(Ordering::SeqCst), 5);
    assert_eq!(fake.freed.load(Ordering::SeqCst), 1);
    assert_eq!(manager.estado.lock().unwrap().pool.len(), 4);

    let reused = manager.adquirir().unwrap();
    assert_eq!(reused.reconocedor.as_ref().unwrap().0, 4);
    assert_eq!(manager.estado.lock().unwrap().pool.len(), 3);

    manager.destroy();
    assert!(!manager.is_ready());
    assert_eq!(fake.freed.load(Ordering::SeqCst), 4);
    drop(reused);
    assert_eq!(fake.freed.load(Ordering::SeqCst), 5);
}
