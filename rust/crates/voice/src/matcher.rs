use crate::comandos::{COMMANDS_WITH_ARGUMENTS, COURTESY_PREFIXES, DEFAULT_BINDINGS};
pub use crate::matcher_texto::{
    clean_query, find_wake_word_occurrences, get_all_wake_words, is_allowed_non_arg_query,
    is_preposition_para, normalizar_texto, parse_wake_words, CommandPhrase, FrasePersonalizada,
    WakeOccurrence,
};

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Coincidencia {
    pub coincide: bool,
    pub motivo: Option<String>,
    pub palabra: Option<String>,
    pub comando: Option<String>,
    pub frase: Option<String>,
    pub consulta: Option<String>,
    pub falta_consulta: Option<bool>,
    pub relleno: Option<String>,
}

impl Coincidencia {
    fn fail(motivo: &str, palabra: Option<String>) -> Self {
        Self {
            coincide: false,
            motivo: Some(motivo.into()),
            palabra,
            ..Default::default()
        }
    }
    fn ok(
        palabra: Option<String>,
        cmd: &str,
        frase: &str,
        query: String,
        missing: bool,
        relleno: Option<String>,
    ) -> Self {
        Self {
            coincide: true,
            motivo: None,
            palabra,
            comando: Some(cmd.into()),
            frase: Some(frase.into()),
            consulta: Some(query),
            falta_consulta: Some(missing),
            relleno,
        }
    }
}

struct MatchedCommand {
    command_id: String,
    phrase: String,
    query: String,
    missing_query: bool,
}

pub fn build_phrase_list(custom: Option<&[FrasePersonalizada]>) -> Vec<CommandPhrase> {
    let mut list = Vec::new();
    for &(cmd, phrases) in DEFAULT_BINDINGS {
        for &p in phrases {
            list.push(CommandPhrase {
                phrase: normalizar_texto(p),
                command_id: cmd.to_string(),
            });
        }
    }
    if let Some(custom_list) = custom {
        for item in custom_list {
            list.push(CommandPhrase {
                phrase: normalizar_texto(&item.frase),
                command_id: item.comando.clone(),
            });
        }
    }
    list.sort_by_key(|a| std::cmp::Reverse(a.phrase.len()));
    list
}

fn match_command_at_start(text: &str, phrases: &[CommandPhrase]) -> Option<MatchedCommand> {
    if text.is_empty() {
        return None;
    }
    for item in phrases {
        if text == item.phrase || text.starts_with(&format!("{} ", item.phrase)) {
            let raw_query = text[item.phrase.len()..].trim();
            let req_arg = COMMANDS_WITH_ARGUMENTS.contains(&item.command_id.as_str());
            if (!req_arg && !is_allowed_non_arg_query(raw_query))
                || is_preposition_para(&item.phrase, raw_query)
            {
                continue;
            }
            let query = if req_arg {
                clean_query(raw_query)
            } else {
                String::new()
            };
            return Some(MatchedCommand {
                command_id: item.command_id.clone(),
                phrase: item.phrase.clone(),
                missing_query: req_arg && query.is_empty(),
                query,
            });
        }
    }
    None
}

fn match_command_in_rest(
    rest: &str,
    phrases: &[CommandPhrase],
) -> Option<(MatchedCommand, Option<String>)> {
    if rest.is_empty() {
        return None;
    }
    if let Some(m) = match_command_at_start(rest, phrases) {
        return Some((m, None));
    }
    for &prefix in COURTESY_PREFIXES {
        if rest == prefix || rest.starts_with(&format!("{} ", prefix)) {
            let after = rest[prefix.len()..].trim();
            if let Some(m) = match_command_at_start(after, phrases) {
                return Some((m, Some(prefix.to_string())));
            }
        }
    }
    None
}

fn is_greeting(rest: &str) -> bool {
    let w: Vec<&str> = rest.split_whitespace().collect();
    w.len() >= 2
        && ((matches!(w[0], "que" | "ke" | "q") && matches!(w[1], "hace" | "haces"))
            || (w[0] == "k" && w[1] == "ase"))
}

fn is_pure_greeting(text: &str) -> bool {
    let w: Vec<&str> = text.split_whitespace().collect();
    w.len() >= 3 && matches!(w[0], "hola" | "ola") && is_greeting(&w[1..].join(" "))
}

pub fn coincidir(
    raw_text: &str,
    wake_words_input: &str,
    custom: Option<&[FrasePersonalizada]>,
) -> Coincidencia {
    let text = normalizar_texto(raw_text);
    let candidates = parse_wake_words(wake_words_input);
    if text.is_empty() {
        return Coincidencia::fail("empty_text", None);
    }
    let occurrences = find_wake_word_occurrences(&text, &candidates);
    let phrases = build_phrase_list(custom);
    if occurrences.is_empty() {
        if is_pure_greeting(&text) {
            return Coincidencia::ok(
                candidates.first().cloned(),
                "hola_que_hace",
                "hola que hace",
                String::new(),
                false,
                None,
            );
        }
        return Coincidencia::fail("wake_word_missing", candidates.first().cloned());
    }
    for occ in &occurrences {
        if (occ.prefix.ends_with("hola") || occ.prefix.ends_with("ola")) && is_greeting(&occ.rest) {
            return Coincidencia::ok(
                Some(occ.wake_word.clone()),
                "hola_que_hace",
                "hola que hace",
                String::new(),
                false,
                None,
            );
        }
        if let Some((cmd, filler)) = match_command_in_rest(&occ.rest, &phrases) {
            return Coincidencia::ok(
                Some(occ.wake_word.clone()),
                &cmd.command_id,
                &cmd.phrase,
                cmd.query,
                cmd.missing_query,
                filler,
            );
        }
    }
    let last = occurrences.last().unwrap();
    if last.rest.is_empty() {
        Coincidencia::fail("only_wake_word", Some(last.wake_word.clone()))
    } else {
        Coincidencia::fail("command_not_found", Some(last.wake_word.clone()))
    }
}
