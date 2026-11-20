use crate::comandos::Contexto;
use crate::respuesta::{responder, EmisorRespuesta, EstadoInteraccion};
use crate::ui::idioma::vista_idioma;

pub async fn ejecutar_language(
    ctx: &Contexto,
    lang: &str,
    emisor: &dyn EmisorRespuesta,
    ei: &EstadoInteraccion,
) {
    let payload = vista_idioma(&ctx.idiomas, lang, &ctx.idiomas.list(), lang);
    let _ = responder(emisor, ei, &payload).await;
}
