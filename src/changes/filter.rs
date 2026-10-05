use super::diff::{Diff, File};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    query: String,
    pub focused: bool,
}

impl Default for Filter {
    fn default() -> Self {
        Self { query: String::new(), focused: true }
    }
}

impl Filter {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop(&mut self) {
        self.query.pop();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Part(String),
    Glob(String),
}

impl Pattern {
    fn new(word: &str) -> Self {
        if word.contains(['*', '?']) { Self::Glob(word.to_string()) } else { Self::Part(word.to_string()) }
    }

    fn matches(&self, path: &str) -> bool {
        match self {
            Self::Part(part) => {
                let fold = smart_fold(part);
                let (part, path) = (chars(part, fold), chars(path, fold));
                path.windows(part.len()).any(|w| w == part.as_slice())
            }
            Self::Glob(pattern) => glob_matches(pattern, path),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
}

impl Query {
    pub fn parse(raw: &str) -> Self {
        let mut query = Self::default();
        for word in raw.split_whitespace() {
            match word.strip_prefix('!') {
                Some(excluded) if !excluded.is_empty() => query.exclude.push(Pattern::new(excluded)),
                _ => query.include.push(Pattern::new(word)),
            }
        }
        query
    }

    pub fn keeps(&self, file: &File) -> bool {
        let paths = [Some(file.path.as_str()), file.old_path.as_deref()];
        let matches = |pattern: &Pattern| paths.iter().flatten().any(|path| pattern.matches(path));
        (self.include.is_empty() || self.include.iter().any(matches)) && !self.exclude.iter().any(matches)
    }
}

pub fn kept(diff: &Diff, raw: &str) -> Vec<bool> {
    let query = Query::parse(raw);
    diff.files.iter().map(|f| query.keeps(f)).collect()
}

fn chars(text: &str, fold: bool) -> Vec<char> {
    text.chars().map(|c| if fold { c.to_lowercase().next().unwrap_or(c) } else { c }).collect()
}

fn smart_fold(pattern: &str) -> bool {
    !pattern.chars().any(char::is_uppercase)
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    let fold = smart_fold(pattern);
    let (pattern, dir) = pattern.strip_suffix('/').map_or((pattern, false), |p| (p, true));
    let (pattern, anchored) = pattern.strip_prefix('/').map_or((pattern, false), |p| (p, true));
    if anchored || pattern.contains('/') {
        let path = chars(path, fold);
        let whole = chars(pattern, fold);
        let below = chars(&format!("{pattern}/**"), fold);
        return (!dir && glob(&whole, &path)) || glob(&below, &path);
    }
    let pattern = chars(pattern, fold);
    let parts: Vec<&str> = path.split('/').collect();
    let last = parts.len().saturating_sub(1);
    parts.iter().enumerate().any(|(i, part)| (!dir || i < last) && glob(&pattern, &chars(part, fold)))
}

fn glob(pattern: &[char], text: &[char]) -> bool {
    match pattern {
        [] => text.is_empty(),
        ['*', '*', rest @ ..] => {
            let starts = (0..=text.len()).filter(|&i| i == 0 || text[i - 1] == '/');
            let skip_slash =
                rest.strip_prefix(&['/']).is_some_and(|after| starts.into_iter().any(|i| glob(after, &text[i..])));
            skip_slash || (0..=text.len()).any(|i| glob(rest, &text[i..]))
        }
        ['*', rest @ ..] => {
            let end = text.iter().position(|c| *c == '/').unwrap_or(text.len());
            (0..=end).any(|i| glob(rest, &text[i..]))
        }
        ['?', rest @ ..] => text.first().is_some_and(|c| *c != '/') && glob(rest, &text[1..]),
        [c, rest @ ..] => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rstest::rstest;

    use super::*;
    use crate::changes::diff;

    fn sample() -> Diff {
        let files = ["src/returns.rs", "src/order.test.js", "README.md"];
        Diff { files: files.map(|path| Arc::new(diff::untracked(path, 2, Some(b"a\n")))).to_vec() }
    }

    #[rstest]
    #[case::a_name_anywhere("*.test.js", "src/cart/total.test.js", true)]
    #[case::not_another_extension("*.test.js", "src/cart/total.js", false)]
    #[case::a_folder_anywhere("vendor", "lib/vendor/x.js", true)]
    #[case::a_trailing_slash_is_a_folder("dist/", "src/dist", false)]
    #[case::a_slash_anchors_to_the_root("src/*.rs", "src/main.rs", true)]
    #[case::one_star_stays_in_its_folder("src/*.rs", "src/ui/main.rs", false)]
    #[case::two_stars_cross_folders("src/**/*.rs", "src/ui/changes/filter.rs", true)]
    #[case::two_stars_match_no_folder("src/**/*.rs", "src/main.rs", true)]
    #[case::a_folder_takes_what_is_inside("src/api/**", "src/api/v1/orders.ts", true)]
    #[case::a_rooted_folder("/docs", "docs/readme.md", true)]
    #[case::question_mark_is_one_character("?.rs", "src/a.rs", true)]
    #[case::lower_case_ignores_case("*.md", "README.MD", true)]
    #[case::an_upper_case_letter_matches_case("README*", "readme.md", false)]
    fn globs_match_like_gitignore(#[case] pattern: &str, #[case] path: &str, #[case] expected: bool) {
        assert_eq!(glob_matches(pattern, path), expected);
    }

    #[rstest]
    #[case::empty_keeps_everything("", [true, true, true])]
    #[case::a_word_is_part_of_the_path("order", [false, true, false])]
    #[case::a_word_can_hold_a_slash("src/re", [true, false, false])]
    #[case::a_word_ignores_case("readme", [false, false, true])]
    #[case::an_upper_case_letter_matches_case("Readme", [false, false, false])]
    #[case::a_glob("*.test.js", [false, true, false])]
    #[case::words_add_up("*.rs *.md", [true, false, true])]
    #[case::an_exclamation_mark_leaves_out("!*.md", [true, true, false])]
    #[case::includes_and_excludes("src/ !*.test.js", [true, false, false])]
    #[case::a_lone_exclamation_mark_is_a_part("!", [false, false, false])]
    fn keeps_the_files_whose_path_matches(#[case] query: &str, #[case] expected: [bool; 3]) {
        assert_eq!(kept(&sample(), query), expected);
    }

    #[test]
    fn a_renamed_file_is_kept_by_its_old_path() {
        let file = File { old_path: Some("legacy/cart.js".into()), ..(*sample().files[0]).clone() };
        assert_eq!(kept(&Diff { files: vec![Arc::new(file)] }, "*.js"), [true]);
    }
}
