use crate::comandos::{ALLOWED_NON_ARG_SUFFIXES, WAKE_WORD_PHONETIC_VARIANTS_IGNITE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrasePersonalizada {
    pub comando: String,
    pub frase: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPhrase {
    pub phrase: String,
    pub command_id: String,
}

pub struct WakeOccurrence {
    pub wake_word: String,
    pub prefix: String,
    pub rest: String,
    pub index: usize,
}

pub fn normalizar_texto(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => out.push('a'),
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => out.push('e'),
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => out.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => out.push('o'),
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => out.push('u'),
            c if c.is_ascii_alphanumeric() || c == 'ñ' || c == 'Ñ' => {
                out.push(c.to_ascii_lowercase())
            }
            _ => out.push(' '),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse_wake_words(input: &str) -> Vec<String> {
    let list: Vec<String> = input
        .split(',')
        .map(normalizar_texto)
        .filter(|w| !w.is_empty())
        .collect();
    if list.is_empty() {
        vec!["ignite".to_string()]
    } else {
        list
    }
}

pub fn get_all_wake_words(candidate: &[String]) -> Vec<String> {
    let mut set: Vec<String> = Vec::new();
    for w in candidate {
        let low = w.to_lowercase();
        if !set.contains(&low) {
            set.push(low);
        }
    }
    for w in set.clone() {
        if w == "ignite" {
            for &v in WAKE_WORD_PHONETIC_VARIANTS_IGNITE {
                if !set.contains(&v.to_string()) {
                    set.push(v.to_string());
                }
            }
        }
        if w.contains(' ') {
            let no_sp = w.replace(' ', "");
            if !set.contains(&no_sp) {
                set.push(no_sp);
            }
        } else if w.len() >= 8 {
            for i in 3..=w.len() - 3 {
                if w.is_char_boundary(i) {
                    let s = format!("{} {}", &w[..i], &w[i..]);
                    if !set.contains(&s) {
                        set.push(s);
                    }
                }
            }
        }
    }
    set.sort_by_key(|a| std::cmp::Reverse(a.len()));
    set
}

pub fn find_wake_word_occurrences(
    text: &str,
    candidate_wake_words: &[String],
) -> Vec<WakeOccurrence> {
    let mut matches = Vec::new();
    let all = get_all_wake_words(candidate_wake_words);
    if all.is_empty() || text.is_empty() {
        return matches;
    }
    let bytes = text.as_bytes();
    let mut idx = 0;
    while idx < text.len() {
        if idx == 0 || bytes[idx - 1] == b' ' {
            let mut found = None;
            for w in &all {
                if text[idx..].starts_with(w.as_str()) {
                    let end = idx + w.len();
                    if end == text.len() || bytes[end] == b' ' {
                        found = Some((w, end));
                        break;
                    }
                }
            }
            if let Some((w, end)) = found {
                let prefix = text[..idx].trim().to_string();
                let rest = text[end..].trim().to_string();
                matches.push(WakeOccurrence {
                    wake_word: w.to_lowercase(),
                    prefix,
                    rest,
                    index: idx,
                });
                idx = end;
                continue;
            }
        }
        idx += 1;
    }
    matches
}

pub fn clean_query(query: &str) -> String {
    let q = query.trim();
    let l = q.to_lowercase();
    let prefixes = [
        "la cancion de ",
        "la cancion ",
        "el tema de ",
        "el tema ",
        "la musica de ",
        "la musica ",
        "a ",
        "de ",
        "por ",
    ];
    for p in prefixes {
        if l.starts_with(p) {
            return q[p.len()..].trim().to_string();
        }
    }
    q.to_string()
}

pub fn is_allowed_non_arg_query(query: &str) -> bool {
    let q = query.trim();
    q.is_empty() || ALLOWED_NON_ARG_SUFFIXES.contains(&q)
}

pub fn is_preposition_para(phrase: &str, query: &str) -> bool {
    if phrase != "para" || query.is_empty() {
        return false;
    }
    let f = query.split_whitespace().next().unwrap_or("");
    f.ends_with("ar")
        || f.ends_with("er")
        || f.ends_with("ir")
        || f.ends_with("le")
        || f.ends_with("les")
        || f.ends_with("me")
        || f.ends_with("te")
        || f.ends_with("se")
        || f.ends_with("nos")
        || matches!(
            f,
            "que"
                | "mi"
                | "ti"
                | "el"
                | "la"
                | "los"
                | "las"
                | "un"
                | "una"
                | "eso"
                | "esto"
                | "nada"
        )
}
