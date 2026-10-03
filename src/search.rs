use crate::picker::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Group,
    Project,
    Workspace,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goto {
    Group(u64),
    Place { project: u64, workspace: Option<u64>, tab: Option<u64> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub kind: Kind,
    pub goto: Goto,
    pub name: String,
    pub context: String,
    pub keys: Vec<String>,
}

pub fn rank(candidates: Vec<Candidate>, query: &str) -> Vec<Candidate> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, Candidate)> = candidates
        .into_iter()
        .filter_map(|c| c.keys.iter().filter_map(|key| key_rank(&key.to_lowercase(), &needle)).min().map(|r| (r, c)))
        .collect();
    hits.sort_by_key(|(rank, c)| (*rank, c.kind));
    hits.into_iter().map(|(_, c)| c).collect()
}

fn key_rank(key: &str, needle: &str) -> Option<u8> {
    if key == needle {
        Some(0)
    } else if key.starts_with(needle) {
        Some(1)
    } else if key.contains(needle) {
        Some(2)
    } else {
        None
    }
}

pub fn find(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let hay: Vec<char> = haystack.chars().map(fold).collect();
    let needle: Vec<char> = needle.trim().chars().map(fold).collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == needle[..]).map(|i| (i, i + needle.len()))
}

#[derive(Debug, Default)]
pub struct Search {
    query: String,
    selected: usize,
    scroll: usize,
}

impl Search {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.reset();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.reset();
    }

    pub fn reset(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn select(&mut self, i: usize) {
        self.selected = i;
    }

    pub fn move_selection(&mut self, delta: isize, len: usize, rows: usize) {
        let Some(last) = len.checked_sub(1) else { return };
        let mut cursor = Cursor { selected: Some(self.selected.min(last)), scroll: self.scroll };
        cursor.move_by(delta, len, rows);
        (self.selected, self.scroll) = (cursor.selected.unwrap_or(0), cursor.scroll);
    }

    pub fn scroll_by(&mut self, delta: isize, len: usize, rows: usize) {
        let mut cursor = Cursor { selected: None, scroll: self.scroll };
        cursor.scroll_by(delta, len, rows);
        self.scroll = cursor.scroll;
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn candidate(kind: Kind, name: &str, keys: &[&str]) -> Candidate {
        Candidate {
            kind,
            goto: Goto::Group(1),
            name: name.into(),
            context: String::new(),
            keys: keys.iter().map(|k| (*k).to_string()).collect(),
        }
    }

    fn names(results: &[Candidate]) -> Vec<&str> {
        results.iter().map(|c| c.name.as_str()).collect()
    }

    mod rank {
        use super::*;

        #[test]
        fn an_empty_query_finds_nothing() {
            assert_eq!(rank(vec![candidate(Kind::Project, "api", &["api"])], "  "), Vec::<Candidate>::new());
        }

        #[test]
        fn leaves_out_what_does_not_match() {
            let found =
                rank(vec![candidate(Kind::Project, "api", &["api"]), candidate(Kind::Project, "web", &["web"])], "we");
            assert_eq!(names(&found), ["web"]);
        }

        #[test]
        fn exact_then_prefix_then_substring() {
            let found = rank(
                vec![
                    candidate(Kind::Project, "my-api", &["my-api"]),
                    candidate(Kind::Project, "api-docs", &["api-docs"]),
                    candidate(Kind::Project, "api", &["api"]),
                ],
                "api",
            );
            assert_eq!(names(&found), ["api", "api-docs", "my-api"]);
        }

        #[test]
        fn projects_then_workspaces_then_tabs_on_a_tie() {
            let found = rank(
                vec![
                    candidate(Kind::Tab, "login-tab", &["login-tab"]),
                    candidate(Kind::Workspace, "login-ws", &["login-ws"]),
                    candidate(Kind::Project, "login-app", &["login-app"]),
                ],
                "login",
            );
            assert_eq!(names(&found), ["login-app", "login-ws", "login-tab"]);
        }

        #[test]
        fn keeps_the_given_order_otherwise() {
            let found = rank(
                vec![
                    candidate(Kind::Tab, "zsh", &["zsh", "feat/b"]),
                    candidate(Kind::Tab, "nvim", &["nvim", "feat/a"]),
                ],
                "feat",
            );
            assert_eq!(names(&found), ["zsh", "nvim"]);
        }

        #[test]
        fn any_key_matches_and_the_best_one_counts() {
            let found = rank(
                vec![
                    candidate(Kind::Workspace, "renamed", &["renamed", "feat/x"]),
                    candidate(Kind::Workspace, "other", &["other", "x"]),
                ],
                "x",
            );
            assert_eq!(names(&found), ["other", "renamed"]);
        }

        #[test]
        fn ignores_case() {
            assert_eq!(names(&rank(vec![candidate(Kind::Project, "API", &["API"])], "api")), ["API"]);
        }
    }

    mod find {
        use super::*;

        #[rstest]
        #[case::start("feat/search", "feat", Some((0, 4)))]
        #[case::middle("my-api", "API", Some((3, 6)))]
        #[case::counts_chars_not_bytes("ñandú-api", "api", Some((6, 9)))]
        #[case::missing("zsh", "vim", None)]
        #[case::empty("zsh", "", None)]
        fn returns_the_char_range_of_the_first_match(
            #[case] haystack: &str,
            #[case] needle: &str,
            #[case] expected: Option<(usize, usize)>,
        ) {
            assert_eq!(find(haystack, needle), expected);
        }
    }

    mod search {
        use super::*;

        #[test]
        fn typing_selects_the_first_result() {
            let mut s = Search::default();
            s.move_selection(2, 5, 10);
            s.push('a');
            assert_eq!((s.query(), s.selected()), ("a", 0));
        }

        #[test]
        fn the_selection_stays_inside_the_results() {
            let mut s = Search::default();
            s.move_selection(10, 3, 10);
            assert_eq!(s.selected(), 2);
        }

        #[test]
        fn moving_down_scrolls_to_keep_the_selection_visible() {
            let mut s = Search::default();
            s.move_selection(4, 10, 3);
            assert_eq!(s.scroll(), 2);
        }

        #[test]
        fn scrolling_stops_at_the_last_page() {
            let mut s = Search::default();
            s.scroll_by(100, 10, 3);
            assert_eq!(s.scroll(), 7);
        }
    }
}
