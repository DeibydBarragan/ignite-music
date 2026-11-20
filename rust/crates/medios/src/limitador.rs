use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prioridad {
    Alta,
    Baja,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LimitadorError {
    #[error("Timed out after {0}s waiting for a free slot")]
    TiempoAgotado(u64),
    #[error("Aborted while waiting for a slot")]
    Cancelado,
}

struct Mesero {
    id: u64,
    prioridad: Prioridad,
    tx: oneshot::Sender<Permiso>,
}

struct LimitadorInterno {
    limit: usize,
    active: usize,
    last_served: Prioridad,
    alta: VecDeque<Mesero>,
    baja: VecDeque<Mesero>,
}

impl LimitadorInterno {
    fn tomar_siguiente(&mut self) -> Option<Mesero> {
        while !self.alta.is_empty() || !self.baja.is_empty() {
            let mesero = if self.last_served == Prioridad::Alta && !self.baja.is_empty() {
                self.baja.pop_front()
            } else if !self.alta.is_empty() {
                self.alta.pop_front()
            } else {
                self.baja.pop_front()
            };

            if let Some(m) = mesero {
                if !m.tx.is_closed() {
                    self.last_served = m.prioridad;
                    return Some(m);
                }
            }
        }
        None
    }
}

#[derive(Clone)]
pub struct Limitador {
    inner: Arc<Mutex<LimitadorInterno>>,
    next_id: Arc<AtomicU64>,
}

impl std::fmt::Debug for Limitador {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Limitador").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Permiso {
    limitador: Limitador,
    activo: bool,
}

impl Permiso {
    pub fn release(mut self) {
        if self.activo {
            self.activo = false;
            self.limitador.liberar_slot();
        }
    }
}

impl Drop for Permiso {
    fn drop(&mut self) {
        if self.activo {
            self.activo = false;
            self.limitador.liberar_slot();
        }
    }
}

impl Limitador {
    pub fn new(limit: usize) -> Self {
        let capacity = limit.max(1);
        Self {
            inner: Arc::new(Mutex::new(LimitadorInterno {
                limit: capacity,
                active: 0,
                last_served: Prioridad::Baja,
                alta: VecDeque::new(),
                baja: VecDeque::new(),
            })),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn active(&self) -> usize {
        self.inner.lock().unwrap().active
    }

    pub fn limit(&self) -> usize {
        self.inner.lock().unwrap().limit
    }

    pub fn pending_high(&self) -> usize {
        self.inner.lock().unwrap().alta.len()
    }

    pub fn pending_low(&self) -> usize {
        self.inner.lock().unwrap().baja.len()
    }

    pub async fn acquire(
        &self,
        prioridad: Prioridad,
        timeout: Option<Duration>,
    ) -> Result<Permiso, LimitadorError> {
        let (id, rx) = {
            let mut state = self.inner.lock().unwrap();
            if state.active < state.limit {
                state.active += 1;
                return Ok(Permiso {
                    limitador: self.clone(),
                    activo: true,
                });
            }

            let (tx, rx) = oneshot::channel();
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let mesero = Mesero { id, prioridad, tx };
            if prioridad == Prioridad::Alta {
                state.alta.push_back(mesero);
            } else {
                state.baja.push_back(mesero);
            }
            (id, rx)
        };

        if let Some(dur) = timeout {
            match tokio::time::timeout(dur, rx).await {
                Ok(Ok(permiso)) => Ok(permiso),
                Ok(Err(_)) => Err(LimitadorError::Cancelado),
                Err(_) => {
                    self.remover_mesero(id, prioridad);
                    let secs = (dur.as_millis() as u64 + 500) / 1000;
                    Err(LimitadorError::TiempoAgotado(secs))
                }
            }
        } else {
            match rx.await {
                Ok(permiso) => Ok(permiso),
                Err(_) => Err(LimitadorError::Cancelado),
            }
        }
    }

    fn remover_mesero(&self, id: u64, prioridad: Prioridad) {
        let mut state = self.inner.lock().unwrap();
        let queue = if prioridad == Prioridad::Alta {
            &mut state.alta
        } else {
            &mut state.baja
        };
        if let Some(pos) = queue.iter().position(|m| m.id == id) {
            queue.remove(pos);
        }
    }

    fn liberar_slot(&self) {
        let mut state = self.inner.lock().unwrap();
        while let Some(mesero) = state.tomar_siguiente() {
            let permiso = Permiso {
                limitador: self.clone(),
                activo: true,
            };
            match mesero.tx.send(permiso) {
                Ok(()) => return,
                Err(mut permiso) => permiso.activo = false,
            }
        }
        state.active = state.active.saturating_sub(1);
    }

    pub async fn run<F, Fut, T>(
        &self,
        prioridad: Prioridad,
        timeout: Option<Duration>,
        f: F,
    ) -> Result<T, LimitadorError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let permiso = self.acquire(prioridad, timeout).await?;
        let result = f().await;
        permiso.release();
        Ok(result)
    }
}
