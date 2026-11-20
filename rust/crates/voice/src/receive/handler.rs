use super::{VoiceFragment, VoiceSegmenter};
use songbird::events::{CoreEvent, Event, EventContext, EventHandler};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct VoiceReceiverHandler {
    segmenter: Arc<Mutex<VoiceSegmenter>>,
    on_fragment: Arc<dyn Fn(VoiceFragment) + Send + Sync>,
    canal: Arc<Mutex<Option<songbird::id::ChannelId>>>,
}

impl VoiceReceiverHandler {
    pub fn new(
        segmenter: Arc<Mutex<VoiceSegmenter>>,
        on_fragment: Arc<dyn Fn(VoiceFragment) + Send + Sync>,
    ) -> Self {
        Self {
            segmenter,
            on_fragment,
            canal: Arc::new(Mutex::new(None)),
        }
    }

    pub fn registrar_en(self, llamada: &mut songbird::Call) {
        for evento in [
            CoreEvent::SpeakingStateUpdate,
            CoreEvent::VoiceTick,
            CoreEvent::ClientDisconnect,
            CoreEvent::DriverConnect,
            CoreEvent::DriverDisconnect,
            CoreEvent::DriverReconnect,
        ] {
            llamada.add_global_event(Event::Core(evento), self.clone());
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for VoiceReceiverHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        match ctx {
            EventContext::SpeakingStateUpdate(s) => {
                let user_id = s.user_id.map(|u| u.0);
                self.segmenter.lock().await.register_ssrc(s.ssrc, user_id);
            }
            EventContext::VoiceTick(tick) => {
                let mut speaking = Vec::new();
                for (&ssrc, data) in &tick.speaking {
                    if let Some(pcm) = &data.decoded_voice {
                        speaking.push((ssrc, pcm.as_slice()));
                    }
                }
                let silent: Vec<u32> = tick.silent.iter().copied().collect();

                let fragments = {
                    let mut seg = self.segmenter.lock().await;
                    seg.process_tick(&speaking, &silent).await
                };

                for frag in fragments {
                    (self.on_fragment)(frag);
                }
            }
            EventContext::ClientDisconnect(d) => {
                let frag = self
                    .segmenter
                    .lock()
                    .await
                    .on_client_disconnect(d.user_id.0);
                if let Some(f) = frag {
                    (self.on_fragment)(f);
                }
            }
            EventContext::DriverDisconnect(_) => {
                let mut segmenter = self.segmenter.lock().await;
                segmenter.active_buffers.clear();
                segmenter.ssrc_to_user.clear();
            }
            EventContext::DriverConnect(datos) => {
                let previo = self.canal.lock().await.replace(datos.channel_id);
                if previo.is_some_and(|canal| canal != datos.channel_id) {
                    let mut segmenter = self.segmenter.lock().await;
                    segmenter.active_buffers.clear();
                    segmenter.ssrc_to_user.clear();
                }
            }
            EventContext::DriverReconnect(_) => {
                self.segmenter.lock().await.active_buffers.clear();
            }
            _ => {}
        }
        None
    }
}
