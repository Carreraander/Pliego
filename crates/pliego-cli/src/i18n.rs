//! Idioma de la CLI. Los crates escriben sus mensajes en español; con el sistema (o `PLIEGO_LANG`) en inglés, la CLI
//! los traduce al imprimirlos con las mismas reglas que la interfaz (`app/src/i18n/engine/`, exportadas a `rules.json`
//! por `cli-rules.test.ts`) más las de la ayuda. Una regla es `[plantilla en español, plantilla en inglés]`; `{}` captura
//! un trozo que también se traduce. Lo que no tiene regla (nombres de archivo, errores libres) se imprime tal cual.

use std::sync::OnceLock;

use clap::Command;
use pliego_core::Lang;
use regex::Regex;

struct Rule {
    re: Regex,
    en: String,
}

static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
static LANG: OnceLock<Lang> = OnceLock::new();

const DATA: &str = include_str!("i18n/rules.json");
const HELP: &str = include_str!("i18n/help.json");

/// Ayuda traducida: coincidencia exacta con el texto español (sin huecos ni reglas generales).
fn help() -> &'static std::collections::HashMap<String, String> {
    static MAP: OnceLock<std::collections::HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let pairs: Vec<(String, String)> = serde_json::from_str(HELP).expect("i18n/help.json no es válido");
        pairs.into_iter().map(|(es, en)| (norm(&es), en)).collect()
    })
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Texto de ayuda en inglés (si no hay traducción, el original).
pub fn help_text(es: &str) -> String {
    help().get(&norm(es)).cloned().unwrap_or_else(|| es.to_string())
}

/// Idioma de la sesión: `PLIEGO_LANG`, si no el del sistema (idioma de Windows o `LC_ALL`/`LANG`); español por defecto.
pub fn lang() -> Lang {
    *LANG.get_or_init(|| {
        if let Some(tag) = std::env::var("PLIEGO_LANG").ok().filter(|v| !v.trim().is_empty()) {
            return Lang::from_tag(&tag);
        }
        match sys_locale::get_locale() {
            Some(tag) => Lang::from_tag(&tag),
            None => Lang::from_env(),
        }
    })
}

fn rules() -> &'static [Rule] {
    RULES.get_or_init(|| {
        let pairs: Vec<(String, String)> = serde_json::from_str(DATA).expect("i18n/rules.json no es válido");
        pairs.into_iter().map(|(es, en)| compile(&es, en)).collect()
    })
}

/// `Fila {} de {}` → `^Fila (.+?) de (.+?)$`.
fn compile(es: &str, en: String) -> Rule {
    let mut pattern = String::from("(?s)^");
    let mut rest = es;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        let name = &rest[start + 1..start + len];
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            // No es un hueco (`{` suelto en el texto): se trata como literal.
            pattern.push_str(&regex::escape(&rest[..start + 1]));
            rest = &rest[start + 1..];
            continue;
        }
        pattern.push_str(&regex::escape(&rest[..start]));
        pattern.push_str("(.+?)");
        rest = &rest[start + len + 1..];
    }
    pattern.push_str(&regex::escape(rest));
    pattern.push('$');
    Rule {
        re: Regex::new(&pattern).unwrap_or_else(|e| panic!("regla no válida «{es}»: {e}")),
        en,
    }
}

/// Rellena `{}` (por orden) y `{n}` (por posición, desde 1) de una plantilla inglesa.
fn fill(template: &str, args: &[String]) -> String {
    let hole = Regex::new(r"\{(\d*)\}").expect("regex fija");
    let mut next = 0;
    hole.replace_all(template, |c: &regex::Captures| {
        let i = match &c[1] {
            "" => {
                next += 1;
                next - 1
            }
            n => n.parse::<usize>().unwrap_or(1).saturating_sub(1),
        };
        args.get(i).cloned().unwrap_or_default()
    })
    .into_owned()
}

/// Traduce un trozo capturado conservando los espacios que lo rodean.
fn part(arg: &str) -> String {
    let body = arg.trim();
    if body.is_empty() {
        return arg.to_string();
    }
    let lead = &arg[..arg.len() - arg.trim_start().len()];
    let tail = &arg[arg.trim_end().len()..];
    format!("{lead}{}{tail}", translate(body))
}

/// Texto traducido al inglés; sin regla que lo cubra, queda tal cual.
pub fn translate(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return text.to_string();
    }
    for rule in rules() {
        if let Some(m) = rule.re.captures(&normalized) {
            let args: Vec<String> = (1..m.len())
                .map(|i| part(m.get(i).map_or("", |g| g.as_str())))
                .collect();
            let out = fill(&rule.en, &args);
            // Una regla que solo reordena espacios (el prefijo genérico sobre un texto sin traducción) no cambia nada.
            return if out == normalized { text.to_string() } else { out };
        }
    }
    text.to_string()
}

/// Traduce `text` si la sesión está en inglés.
pub fn tr(text: &str) -> String {
    match lang() {
        Lang::Es => text.to_string(),
        Lang::En => translate(text),
    }
}

/// Traduce un bloque línea a línea, respetando la sangría. No toca nada en español ni con `--json`.
pub fn tr_lines(text: &str) -> String {
    if lang() == Lang::Es || JSON_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        return text.to_string();
    }
    text.split('\n')
        .map(|line| {
            let body = line.trim_start();
            if body.is_empty() {
                return line.to_string();
            }
            let indent = &line[..line.len() - body.len()];
            format!("{indent}{}", translate(body))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

static JSON_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Con `--json` la salida es para máquinas: se deja como la escribe el motor.
pub fn set_json(on: bool) {
    JSON_MODE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Traduce la ayuda (`about`, `long_about`, texto de cada opción) de todo el árbol de comandos.
pub fn localize(cmd: Command) -> Command {
    if lang() == Lang::Es {
        return cmd;
    }
    localize_with(cmd, &help_text)
}

pub(crate) fn localize_with(mut cmd: Command, f: &dyn Fn(&str) -> String) -> Command {
    if let Some(a) = cmd.get_about().map(|s| s.to_string()) {
        cmd = cmd.about(f(&a));
    }
    if let Some(a) = cmd.get_long_about().map(|s| s.to_string()) {
        cmd = cmd.long_about(paragraphs(&a, f));
    }
    let ids: Vec<_> = cmd.get_arguments().map(|a| a.get_id().clone()).collect();
    for id in ids {
        cmd = cmd.mut_arg(id, |mut a| {
            if let Some(h) = a.get_help().map(|s| s.to_string()) {
                a = a.help(f(&h));
            }
            if let Some(h) = a.get_long_help().map(|s| s.to_string()) {
                a = a.long_help(paragraphs(&h, f));
            }
            a
        });
    }
    let subs: Vec<String> = cmd.get_subcommands().map(|s| s.get_name().to_string()).collect();
    for name in subs {
        cmd = cmd.mut_subcommand(name, |s| localize_with(s, f));
    }
    cmd
}

fn paragraphs(text: &str, f: &dyn Fn(&str) -> String) -> String {
    text.split("\n\n").map(f).collect::<Vec<_>>().join("\n\n")
}

/// Textos de ayuda de todo el árbol (para la prueba que exige que tengan traducción).
#[cfg(test)]
pub(crate) fn help_texts(cmd: &Command, out: &mut Vec<(String, String)>) {
    let path = cmd.get_name().to_string();
    for (kind, s) in [("about", cmd.get_about()), ("long_about", cmd.get_long_about())] {
        if let Some(s) = s {
            out.push((format!("{path} {kind}"), s.to_string()));
        }
    }
    for a in cmd.get_arguments() {
        for (kind, s) in [("help", a.get_help()), ("long_help", a.get_long_help())] {
            if let Some(s) = s {
                out.push((format!("{path} --{} {kind}", a.get_id()), s.to_string()));
            }
        }
    }
    for s in cmd.get_subcommands() {
        help_texts(s, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<(String, String)> {
        let mut out = Vec::new();
        help_texts(&<crate::Cli as clap::CommandFactory>::command(), &mut out);
        out
    }

    /// Toda la ayuda de `pliego --help` tiene traducción al inglés: al añadir un subcomando u opción, añade su regla en
    /// `app/src/i18n/engine/rules-cli-help.ts` y regenera el JSON (`UPDATE_CLI_RULES=1 npx vitest run cli-rules`).
    #[test]
    fn all_help_text_is_translated() {
        let missing: Vec<String> = tree()
            .into_iter()
            .filter(|(_, es)| help_text(es) == *es)
            .map(|(at, es)| format!("{at}: {es}"))
            .collect();
        if let Ok(path) = std::env::var("DUMP_MISSING_HELP") {
            let mut uniq: Vec<String> = missing
                .iter()
                .filter_map(|m| m.split_once(": ").map(|x| x.1.to_string()))
                .collect();
            uniq.sort();
            uniq.dedup();
            std::fs::write(path, uniq.join("\n")).unwrap();
        }
        assert!(
            missing.is_empty(),
            "ayuda sin traducir ({}):\n{}",
            missing.len(),
            missing.join("\n")
        );
    }

    #[test]
    fn translates_with_the_shared_rules() {
        assert_eq!(translate("Fila 3 de 10"), "Row 3 of 10");
        assert_eq!(translate("  texto sin regla  "), "  texto sin regla  ");
    }

    #[test]
    fn localizing_the_help_replaces_the_descriptions() {
        let cmd = localize_with(<crate::Cli as clap::CommandFactory>::command(), &|s| format!("<{s}>"));
        let about = cmd
            .get_subcommands()
            .find(|s| s.get_name() == "info")
            .and_then(|s| s.get_about())
            .map(|s| s.to_string());
        assert_eq!(about.as_deref(), Some("<Información del documento>"));
    }
}
