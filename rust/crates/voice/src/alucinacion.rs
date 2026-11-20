pub const FRASES_ALUCINACION: &[&str] = &[
    "gracias",
    "muchas gracias",
    "gracias por ver el video",
    "gracias por ver",
    "thank you",
    "thanks",
    "thanks for watching",
    "thank you for watching",
    "suscribete",
    "suscribete al canal",
    "no olvides suscribirte",
    "subtitulos por la comunidad de amara org",
    "subtitulado por la comunidad de amara org",
    "subtitulos realizados por la comunidad de amara org",
    "amara org",
    "you",
    "bye",
    "adios",
    "hasta la proxima",
];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub struct SegmentoWhisper {
    #[serde(default)]
    pub no_speech_prob: f64,
    #[serde(default)]
    pub avg_logprob: f64,
}

pub fn normalizar_alucinacion(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => out.push('a'),
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => out.push('e'),
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => out.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => out.push('o'),
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => out.push('u'),
            'ñ' | 'Ñ' => out.push('n'),
            c if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
            _ => out.push(' '),
        }
    }
    let words: Vec<&str> = out.split_whitespace().collect();
    words.join(" ")
}

pub fn es_alucinacion(texto: &str, segmentos: &[SegmentoWhisper]) -> bool {
    let norm = normalizar_alucinacion(texto);
    if norm.is_empty() {
        return true;
    }
    if FRASES_ALUCINACION.contains(&norm.as_str()) {
        return true;
    }
    if segmentos.is_empty() {
        return false;
    }
    segmentos
        .iter()
        .all(|s| s.no_speech_prob > 0.6 && s.avg_logprob < -1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalizar_alucinacion() {
        assert_eq!(normalizar_alucinacion("  ¡Suscríbete!  "), "suscribete");
        assert_eq!(normalizar_alucinacion("Gracias."), "gracias");
        assert_eq!(
            normalizar_alucinacion("Subtítulos por la comunidad de Amara.org"),
            "subtitulos por la comunidad de amara org"
        );
    }

    #[test]
    fn test_es_alucinacion_unitarios() {
        assert!(es_alucinacion("", &[]));
        assert!(es_alucinacion("Gracias.", &[]));
        assert!(es_alucinacion("¡Suscríbete!", &[]));
        assert!(es_alucinacion(
            "Subtítulos por la comunidad de Amara.org",
            &[]
        ));
        assert!(es_alucinacion("Thank you.", &[]));
        assert!(!es_alucinacion("ignite pausa", &[]));

        let silent = vec![SegmentoWhisper {
            no_speech_prob: 0.9,
            avg_logprob: -1.5,
        }];
        assert!(es_alucinacion("ignite pon algo", &silent));

        let real = vec![SegmentoWhisper {
            no_speech_prob: 0.05,
            avg_logprob: -0.2,
        }];
        assert!(!es_alucinacion("ignite pon algo", &real));

        let mixed = vec![
            SegmentoWhisper {
                no_speech_prob: 0.9,
                avg_logprob: -1.5,
            },
            SegmentoWhisper {
                no_speech_prob: 0.05,
                avg_logprob: -0.2,
            },
        ];
        assert!(!es_alucinacion("ignite pon algo", &mixed));
    }
}
