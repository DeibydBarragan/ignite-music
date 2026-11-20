use crate::audio::pcm_rms;

pub const WAKE_DECOYS_IGNITE: &[&str] = &["ignore", "ignored", "ignores", "excite", "invite"];
pub const FILLER_WORDS: &[&str] = &["oye", "hey"];
pub const UNKNOWN: &str = "[unk]";
pub const MODEL_ORDER: &[&str] = &["en", "es"];

pub const TAIL_VOICE_RMS: f64 = 400.0;
pub const TAIL_SKIP_SEC: f64 = 0.1;
pub const TAIL_WINDOW_SAMPLES: usize = 1600;

pub fn split_compound(word: &str, known: impl Fn(&str) -> bool) -> Option<(String, String)> {
    if word.len() < 4 {
        return None;
    }
    for i in (2..=word.len() - 2).rev() {
        if !word.is_char_boundary(i) {
            continue;
        }
        let (left, right) = word.split_at(i);
        if known(left) && known(right) {
            return Some((left.to_string(), right.to_string()));
        }
    }
    None
}

pub fn has_voiced_tail(pcm16k: &[i16], from_sec: f64) -> bool {
    let window = TAIL_WINDOW_SAMPLES;
    let start_sample = (from_sec * 16000.0).floor() as usize;
    let mut offset = start_sample;
    while offset + window <= pcm16k.len() {
        if pcm_rms(&pcm16k[offset..offset + window]) >= TAIL_VOICE_RMS {
            return true;
        }
        offset += window;
    }
    false
}

pub fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => out.push('a'),
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => out.push('e'),
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => out.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => out.push('o'),
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => out.push('u'),
            c if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
            _ => out.push(' '),
        }
    }
    let words: Vec<&str> = out.split_whitespace().collect();
    words.join(" ")
}

pub fn parse_wake_words(input: &str) -> Vec<String> {
    let list: Vec<String> = input
        .split(',')
        .map(normalize_text)
        .filter(|w| !w.is_empty())
        .collect();
    if list.is_empty() {
        vec!["ignite".to_string()]
    } else {
        list
    }
}

pub fn tokens_for(wake_word: &str, is_known: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    for part in wake_word.split_whitespace() {
        if is_known(part) {
            tokens.push(part.to_string());
        } else if let Some((l, r)) = split_compound(part, &is_known) {
            tokens.push(l);
            tokens.push(r);
        } else {
            return None;
        }
    }
    if tokens.is_empty() {
        None
    } else {
        Some(tokens)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakePhrase {
    pub wake_word: String,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPlan {
    pub model_id: String,
    pub grammar: Vec<String>,
    pub phrases: Vec<WakePhrase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarPlanResult {
    pub plans: Vec<ModelPlan>,
    pub supported: Vec<String>,
    pub unsupported: Vec<String>,
}

pub fn build_grammar(
    wake_word_input: &str,
    is_known: impl Fn(&str, &str) -> bool,
    has_model: impl Fn(&str) -> bool,
) -> GrammarPlanResult {
    let mut plans_order: Vec<String> = Vec::new();
    let mut plans_entries: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut plans_phrases: std::collections::HashMap<String, Vec<WakePhrase>> =
        std::collections::HashMap::new();
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();

    for wake_word in parse_wake_words(wake_word_input) {
        let mut chosen_model_id = None;
        let mut fallback_model_id = None;

        for &model_id in MODEL_ORDER {
            if !has_model(model_id) {
                continue;
            }
            let Some(_tokens) = tokens_for(&wake_word, |w| is_known(model_id, w)) else {
                continue;
            };

            let all_direct = wake_word
                .split_whitespace()
                .all(|part| is_known(model_id, part));
            if all_direct {
                chosen_model_id = Some(model_id);
                break;
            }
            if fallback_model_id.is_none() {
                fallback_model_id = Some(model_id);
            }
        }

        let Some(model_id) = chosen_model_id.or(fallback_model_id) else {
            unsupported.push(wake_word);
            continue;
        };

        supported.push(wake_word.clone());
        if !plans_entries.contains_key(model_id) {
            plans_order.push(model_id.to_string());
            plans_entries.insert(model_id.to_string(), Vec::new());
            plans_phrases.insert(model_id.to_string(), Vec::new());
        }

        let tokens = tokens_for(&wake_word, |w| is_known(model_id, w)).unwrap();
        let phrase_str = tokens.join(" ");
        let entries = plans_entries.get_mut(model_id).unwrap();
        if !entries.contains(&phrase_str) {
            entries.push(phrase_str);
        }
        plans_phrases.get_mut(model_id).unwrap().push(WakePhrase {
            wake_word: wake_word.clone(),
            tokens,
        });

        if wake_word == "ignite" {
            for &decoy in WAKE_DECOYS_IGNITE {
                if is_known(model_id, decoy) && !entries.contains(&decoy.to_string()) {
                    entries.push(decoy.to_string());
                }
            }
        }
    }

    let plans = plans_order
        .into_iter()
        .map(|model_id| {
            let mut entries = plans_entries.remove(&model_id).unwrap_or_default();
            for &filler in FILLER_WORDS {
                if is_known(&model_id, filler) && !entries.contains(&filler.to_string()) {
                    entries.push(filler.to_string());
                }
            }
            entries.push(UNKNOWN.to_string());
            ModelPlan {
                grammar: entries,
                phrases: plans_phrases.remove(&model_id).unwrap_or_default(),
                model_id,
            }
        })
        .collect();

    GrammarPlanResult {
        plans,
        supported,
        unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_compound() {
        let d = |w: &str| matches!(w, "ignite" | "bot" | "test");
        assert_eq!(
            split_compound("ignitebot", d),
            Some(("ignite".into(), "bot".into()))
        );
        assert_eq!(split_compound("bot", d), None);
        assert_eq!(split_compound("unknownword", d), None);
    }

    #[test]
    fn test_has_voiced_tail() {
        assert!(!has_voiced_tail(&[0i16; 3200], 0.0));
        let mut loud = vec![0i16; 4800];
        loud[1600..3200].fill(5000);
        assert!(has_voiced_tail(&loud, 0.1));
        assert!(!has_voiced_tail(&loud, 0.3));
    }

    #[test]
    fn test_build_grammar_ignite_structure() {
        let is_known = |m: &str, w: &str| {
            m == "en" && matches!(w, "ignite" | "ignore" | "excite" | "oye" | "hey")
        };
        let res = build_grammar("ignite", is_known, |m| m == "en");
        assert_eq!(res.supported, vec!["ignite"]);
        assert!(res.unsupported.is_empty() && res.plans.len() == 1);
        let g = &res.plans[0].grammar;
        assert!(
            g.contains(&"ignore".into())
                && g.contains(&"[unk]".into())
                && !g.contains(&"bot".into())
        );
    }
}
