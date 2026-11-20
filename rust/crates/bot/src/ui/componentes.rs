use crate::respuesta::PayloadRespuesta;
use nucleo::formato::{truncar, url_segura};
use twilight_model::channel::message::component::{
    ActionRow, Button, ButtonStyle, Component, Container, Section, Separator, SeparatorSpacingSize,
    TextDisplay, Thumbnail, UnfurledMediaItem,
};
use twilight_model::channel::message::{AllowedMentions, MessageFlags};

pub mod id {
    pub const PREVIOUS: &str = "m:prev";
    pub const PAUSE: &str = "m:pause";
    pub const SKIP: &str = "m:skip";
    pub const STOP: &str = "m:stop";
    pub const VOLUME: &str = "m:vol";
    pub const SHUFFLE: &str = "m:shuffle";
    pub const LOOP: &str = "m:loop";
    pub const AUTOPLAY: &str = "m:auto";
    pub const QUEUE: &str = "m:queue";
    pub const LYRICS: &str = "m:lyrics";
    pub const FILTER: &str = "m:filter";
    pub const FAVORITE: &str = "m:fav";
    pub const VOLUME_MODAL: &str = "m:volmodal";
    pub const VOLUME_INPUT: &str = "volume";
    pub const QUEUE_PAGE: &str = "q:";
    pub const LYRICS_PAGE: &str = "ly:";
    pub const SEARCH: &str = "s:";
    pub const LANGUAGE: &str = "lang:set";
}

pub const DEFAULT_COLOR: u32 = 0x5865F2;

pub fn texto(contenido: impl Into<String>) -> Component {
    Component::TextDisplay(TextDisplay {
        id: None,
        content: contenido.into(),
    })
}

pub fn separador(grande: bool) -> Component {
    Component::Separator(Separator {
        id: None,
        divider: Some(true),
        spacing: Some(if grande {
            SeparatorSpacingSize::Large
        } else {
            SeparatorSpacingSize::Small
        }),
    })
}

pub fn contenedor(color: Option<u32>, componentes: Vec<Component>) -> Component {
    Component::Container(Container {
        id: None,
        accent_color: Some(Some(color.unwrap_or(DEFAULT_COLOR))),
        spoiler: None,
        components: componentes,
    })
}

pub fn boton(custom_id: &str, label: &str, style: ButtonStyle, disabled: bool) -> Component {
    Component::Button(Button {
        custom_id: Some(custom_id.to_string()),
        disabled,
        emoji: None,
        id: None,
        label: Some(truncar(label, 80)),
        sku_id: None,
        style,
        url: None,
    })
}

pub fn boton_enlace(label: &str, url: &str) -> Component {
    Component::Button(Button {
        custom_id: None,
        disabled: false,
        emoji: None,
        id: None,
        label: Some(truncar(label, 80)),
        sku_id: None,
        style: ButtonStyle::Link,
        url: Some(url.to_string()),
    })
}

pub fn paginador(
    prefix: &str,
    pagina: i64,
    total_paginas: usize,
    etiqueta_atras: &str,
    etiqueta_siguiente: &str,
) -> Component {
    let pages = total_paginas as i64;
    fila(vec![
        boton(
            &format!("{prefix}{}", pagina - 1),
            etiqueta_atras,
            ButtonStyle::Secondary,
            pagina <= 0,
        ),
        boton(
            &format!("{prefix}{}", pagina + 1),
            etiqueta_siguiente,
            ButtonStyle::Secondary,
            pagina >= pages - 1,
        ),
    ])
}

pub fn fila(componentes: Vec<Component>) -> Component {
    Component::ActionRow(ActionRow {
        id: None,
        components: componentes,
    })
}

pub fn titular(c: &mut Vec<Component>, content: String, thumbnail: Option<&str>) {
    if let Some(url) = thumbnail.and_then(url_segura) {
        c.push(Component::Section(Section {
            id: None,
            components: vec![texto(content)],
            accessory: Box::new(Component::Thumbnail(Thumbnail {
                id: None,
                media: UnfurledMediaItem {
                    url,
                    proxy_url: None,
                    height: None,
                    width: None,
                    content_type: None,
                },
                description: None,
                spoiler: None,
            })),
        }));
    } else {
        c.push(texto(content));
    }
}

pub fn vista_base(componentes: Vec<Component>, efimero: bool) -> PayloadRespuesta {
    let mut flags = MessageFlags::IS_COMPONENTS_V2;
    if efimero {
        flags |= MessageFlags::EPHEMERAL;
    }
    PayloadRespuesta {
        content: None,
        components: componentes,
        flags: Some(flags),
        allowed_mentions: Some(AllowedMentions {
            parse: Vec::new(),
            replied_user: false,
            roles: Vec::new(),
            users: Vec::new(),
        }),
    }
}

pub fn aviso(mensaje: &str, efimero: bool, color: Option<u32>) -> PayloadRespuesta {
    let contenido = truncar(mensaje, 3900);
    let c = contenedor(color, vec![texto(contenido)]);
    vista_base(vec![c], efimero)
}
