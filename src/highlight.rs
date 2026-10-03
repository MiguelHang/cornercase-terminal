use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use ratatui::style::{Color, Modifier, Style};
use syntect::easy::ScopeRangeIterator;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

pub type Highlighted = Arc<Vec<Vec<(String, Style)>>>;

const CACHE_SIZE: usize = 256;
const MAX_CODE: usize = 50_000;
const PLAIN: Style = Style::new();
const RULES: [(&str, Style); 22] = [
    ("comment", Style::new().fg(Color::DarkGray)),
    ("keyword.operator", PLAIN),
    ("keyword", Style::new().fg(Color::Magenta)),
    ("storage", Style::new().fg(Color::Magenta)),
    ("constant.language", Style::new().fg(Color::Magenta)),
    ("string", Style::new().fg(Color::Green)),
    ("constant.character", Style::new().fg(Color::Green)),
    ("markup.inserted", Style::new().fg(Color::Green)),
    ("constant.numeric", Style::new().fg(Color::Yellow)),
    ("constant.other.symbol", Style::new().fg(Color::Yellow)),
    ("markup.list", Style::new().fg(Color::Yellow)),
    ("constant.other", PLAIN),
    ("entity.name", Style::new().fg(Color::Cyan)),
    ("entity.other.attribute-name", Style::new().fg(Color::Cyan)),
    ("entity.other.inherited-class", Style::new().fg(Color::Cyan)),
    ("support.function", Style::new().fg(Color::Cyan)),
    ("support.type", Style::new().fg(Color::Cyan)),
    ("support.class", Style::new().fg(Color::Cyan)),
    ("markup.deleted", Style::new().fg(Color::Red)),
    ("markup.heading", Style::new().add_modifier(Modifier::BOLD)),
    ("markup.bold", Style::new().add_modifier(Modifier::BOLD)),
    ("markup.italic", Style::new().add_modifier(Modifier::ITALIC)),
];
const ALIASES: [(&str, &str); 15] = [
    ("ts", "js"),
    ("tsx", "js"),
    ("typescript", "js"),
    ("jsx", "js"),
    ("mjs", "js"),
    ("cjs", "js"),
    ("javascript", "js"),
    ("shell", "bash"),
    ("console", "bash"),
    ("zsh", "bash"),
    ("jsonc", "json"),
    ("json5", "json"),
    ("vue", "html"),
    ("svelte", "html"),
    ("golang", "go"),
];

fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn rules() -> &'static [(Scope, Style)] {
    static PARSED: OnceLock<Vec<(Scope, Style)>> = OnceLock::new();
    PARSED.get_or_init(|| RULES.iter().filter_map(|(name, style)| Scope::new(name).ok().map(|s| (s, *style))).collect())
}

fn cache() -> &'static Mutex<HashMap<u64, Highlighted>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Highlighted>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn language_token(info: &str) -> Option<String> {
    let token = info.split(|c: char| c.is_whitespace() || c == ',' || c == '{').next()?.trim().to_lowercase();
    if token.is_empty() {
        return None;
    }
    Some(ALIASES.iter().find(|(alias, _)| *alias == token).map_or(token, |(_, to)| (*to).to_string()))
}

fn syntax(token: &str) -> Option<&'static SyntaxReference> {
    let set = syntaxes();
    set.find_syntax_by_token(token).or_else(|| set.syntaxes().iter().find(|s| s.name.eq_ignore_ascii_case(token)))
}

pub fn highlight(code: &str, info: &str) -> Option<Highlighted> {
    let token = language_token(info)?;
    if code.len() > MAX_CODE {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    (&token, code).hash(&mut hasher);
    let key = hasher.finish();
    if let Some(found) = cache().lock().get(&key) {
        return Some(Arc::clone(found));
    }
    let highlighted = Arc::new(parse(code, syntax(&token)?)?);
    let mut cache = cache().lock();
    if cache.len() >= CACHE_SIZE {
        cache.clear();
    }
    cache.insert(key, Arc::clone(&highlighted));
    Some(highlighted)
}

fn parse(code: &str, syntax: &SyntaxReference) -> Option<Vec<Vec<(String, Style)>>> {
    let set = syntaxes();
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(code) {
        let ops = state.parse_line(line, set).ok()?;
        let mut spans: Vec<(String, Style)> = Vec::new();
        for (range, op) in ScopeRangeIterator::new(&ops, line) {
            stack.apply(op).ok()?;
            let text = line[range].trim_end_matches(['\n', '\r']).replace('\t', "    ");
            if text.is_empty() {
                continue;
            }
            let style = style_of(&stack);
            match spans.last_mut() {
                Some((last, s)) if *s == style => last.push_str(&text),
                _ => spans.push((text, style)),
            }
        }
        lines.push(spans);
    }
    Some(lines)
}

fn style_of(stack: &ScopeStack) -> Style {
    stack
        .as_slice()
        .iter()
        .rev()
        .find_map(|scope| rules().iter().find(|(rule, _)| rule.is_prefix_of(*scope)).map(|(_, style)| *style))
        .unwrap_or(PLAIN)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn style_of_word(code: &str, info: &str, word: &str) -> Style {
        highlight(code, info)
            .expect("highlighted")
            .iter()
            .flatten()
            .find(|(text, _)| text.contains(word))
            .map(|(_, style)| *style)
            .expect("the word is there")
    }

    #[rstest]
    #[case::keyword("fn main() {}", "rust", "fn", Color::Magenta)]
    #[case::function_name("fn main() {}", "rust", "main", Color::Cyan)]
    #[case::string("let s = \"hi\";", "rust", "hi", Color::Green)]
    #[case::number("x = 42", "python", "42", Color::Yellow)]
    #[case::comment("# note\nx = 1", "py", "note", Color::DarkGray)]
    #[case::typescript_as_javascript("const a = 'x'", "ts", "const", Color::Magenta)]
    #[case::diff_addition("+added line\n-removed line", "diff", "added", Color::Green)]
    #[case::diff_deletion("+added line\n-removed line", "diff", "removed", Color::Red)]
    fn colours_tokens_with_the_terminal_palette(
        #[case] code: &str,
        #[case] info: &str,
        #[case] word: &str,
        #[case] colour: Color,
    ) {
        assert_eq!(style_of_word(code, info, word).fg, Some(colour));
    }

    #[test]
    fn operators_stay_plain() {
        assert_eq!(style_of_word("let x = 1 + 2;", "rust", "+").fg, None);
    }

    #[test]
    fn keeps_one_line_per_source_line_without_newlines() {
        let lines = highlight("a = 1\nb = 2\n", "python").expect("highlighted");
        let text: Vec<String> = lines.iter().map(|l| l.iter().map(|(t, _)| t.as_str()).collect()).collect();
        assert_eq!(text, ["a = 1", "b = 2"]);
    }

    #[rstest]
    #[case::extra_words("rust ignore", Some("rust"))]
    #[case::attributes("js {1,3}", Some("js"))]
    #[case::case("Python", Some("python"))]
    #[case::alias("tsx", Some("js"))]
    #[case::empty("  ", None)]
    fn reads_the_language_from_the_fence(#[case] info: &str, #[case] expected: Option<&str>) {
        assert_eq!(language_token(info).as_deref(), expected);
    }

    #[test]
    fn an_unknown_language_is_not_highlighted() {
        assert!(highlight("x", "no-such-language").is_none());
    }

    #[test]
    fn the_same_block_comes_from_the_cache() {
        let first = highlight("fn cached() {}", "rust").expect("highlighted");
        let second = highlight("fn cached() {}", "rust").expect("highlighted");
        assert!(Arc::ptr_eq(&first, &second));
    }
}
