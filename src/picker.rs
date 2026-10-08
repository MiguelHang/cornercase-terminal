use std::io;
use std::path::{Path, PathBuf};

use crate::{git, vscode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub workspace: bool,
}

#[derive(Debug)]
pub struct Picker {
    dir: PathBuf,
    home: Option<PathBuf>,
    parent: Option<Item>,
    folders: Vec<Item>,
    filter: String,
    selected: Option<usize>,
    scroll: usize,
    error: Option<String>,
}

impl Picker {
    pub fn open(dir: &Path, home: Option<&Path>) -> io::Result<Self> {
        let parent =
            dir.parent().map(|p| Item { name: "..".into(), path: p.to_path_buf(), branch: None, workspace: false });
        Ok(Self {
            dir: dir.to_path_buf(),
            home: home.map(Path::to_path_buf),
            parent,
            folders: entries(dir)?,
            filter: String::new(),
            selected: None,
            scroll: 0,
            error: None,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn items(&self) -> Vec<&Item> {
        let needle = self.filter.to_lowercase();
        let show_hidden = self.filter.starts_with('.');
        let mut matches: Vec<(u8, &Item)> = self
            .folders
            .iter()
            .filter(|f| show_hidden || !f.name.starts_with('.'))
            .filter_map(|f| {
                let name = f.name.to_lowercase();
                let rank = if name == needle {
                    0
                } else if name.starts_with(&needle) {
                    1
                } else if name.contains(&needle) {
                    2
                } else {
                    return None;
                };
                Some((rank, f))
            })
            .collect();
        matches.sort_by_key(|(rank, item)| (item.workspace, *rank));
        let parent = self.parent.as_ref().filter(|_| self.filter.is_empty());
        parent.into_iter().chain(matches.into_iter().map(|(_, f)| f)).collect()
    }

    pub fn push(&mut self, c: char) {
        self.error = None;
        match c {
            '/' if self.filter.is_empty() && self.selected.is_none() => self.go(Path::new("/")),
            '/' => self.enter_selected(),
            '~' if self.filter.is_empty() => {
                if let Some(home) = self.home.clone() {
                    self.go(&home);
                }
            }
            _ => {
                self.filter.push(c);
                self.selected = (!self.items().is_empty()).then_some(0);
                self.scroll = 0;
            }
        }
    }

    pub fn pop(&mut self) {
        self.error = None;
        if self.filter.pop().is_none() {
            self.up();
            return;
        }
        self.selected = (!self.filter.is_empty() && !self.items().is_empty()).then_some(0);
        self.scroll = 0;
    }

    pub fn up(&mut self) {
        if let Some(parent) = self.dir.parent().map(Path::to_path_buf) {
            self.go(&parent);
        }
    }

    pub fn enter(&mut self, i: usize) {
        if let Some(path) = self.items().get(i).filter(|item| !item.workspace).map(|item| item.path.clone()) {
            self.go(&path);
        }
    }

    pub fn choose(&mut self, i: usize) -> Option<PathBuf> {
        let item = self.items().get(i).map(|item| (item.workspace, item.path.clone()));
        match item {
            Some((true, path)) => Some(path),
            Some((false, path)) => {
                self.go(&path);
                None
            }
            None => None,
        }
    }

    pub fn fail(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn enter_selected(&mut self) {
        if let Some(i) = self.selected {
            self.enter(i);
        }
    }

    pub fn submit(&mut self) -> Option<PathBuf> {
        match self.selected {
            Some(i) => self.choose(i),
            None if self.filter.is_empty() => Some(self.dir.clone()),
            None => None,
        }
    }

    pub fn move_selection(&mut self, delta: isize, rows: usize) {
        let mut cursor = Cursor { selected: self.selected, scroll: self.scroll };
        cursor.move_by(delta, self.items().len(), rows);
        (self.selected, self.scroll) = (cursor.selected, cursor.scroll);
    }

    pub fn scroll_by(&mut self, delta: isize, rows: usize) {
        let mut cursor = Cursor { selected: self.selected, scroll: self.scroll };
        cursor.scroll_by(delta, self.items().len(), rows);
        self.scroll = cursor.scroll;
    }

    fn go(&mut self, dir: &Path) {
        match Self::open(dir, self.home.as_deref()) {
            Ok(picker) => *self = picker,
            Err(e) => self.error = Some(format!("cannot open {}: {e}", dir.display())),
        }
    }
}

fn entries(dir: &Path) -> io::Result<Vec<Item>> {
    let mut entries: Vec<Item> = std::fs::read_dir(dir)?
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| {
            let workspace = vscode::is_workspace(&path) && path.is_file();
            if !workspace && !path.is_dir() {
                return None;
            }
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let branch = if path.join(".git").exists() { git::branch(&path) } else { None };
            Some(Item { name, path, branch, workspace })
        })
        .collect();
    entries.sort_by(|a, b| {
        a.workspace
            .cmp(&b.workspace)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub selected: Option<usize>,
    pub scroll: usize,
}

impl Cursor {
    pub fn move_by(&mut self, delta: isize, len: usize, rows: usize) {
        let Some(last) = len.checked_sub(1) else { return };
        let i = match self.selected {
            Some(i) => i.saturating_add_signed(delta).min(last),
            None if delta < 0 => last,
            None => 0,
        };
        self.selected = Some(i);
        if i < self.scroll {
            self.scroll = i;
        } else if rows > 0 && i >= self.scroll + rows {
            self.scroll = i + 1 - rows;
        }
    }

    pub fn scroll_by(&mut self, delta: isize, len: usize, rows: usize) {
        let max = len.saturating_sub(rows);
        self.scroll = self.scroll.min(max).saturating_add_signed(delta).min(max);
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_util::{TempDir, git_repo};

    fn tree(folders: &[&str]) -> TempDir {
        let tmp = TempDir::new();
        for folder in folders {
            std::fs::create_dir_all(tmp.path().join(folder)).expect("create folder");
        }
        tmp
    }

    fn open(dir: &Path) -> Picker {
        Picker::open(dir, None).expect("open picker")
    }

    fn names(picker: &Picker) -> Vec<&str> {
        picker.items().into_iter().map(|item| item.name.as_str()).collect()
    }

    fn type_text(picker: &mut Picker, text: &str) {
        for c in text.chars() {
            picker.push(c);
        }
    }

    mod listing {
        use super::*;

        #[test]
        fn shows_the_parent_then_the_folders_sorted() {
            let tmp = tree(&["beta", "Alpha", "gamma"]);
            assert_eq!(names(&open(tmp.path())), ["..", "Alpha", "beta", "gamma"]);
        }

        #[test]
        fn leaves_out_files() {
            let tmp = tree(&["src"]);
            std::fs::write(tmp.path().join("README"), "hi").expect("write file");
            assert_eq!(names(&open(tmp.path())), ["..", "src"]);
        }

        #[test]
        fn hides_hidden_folders() {
            let tmp = tree(&[".git", "src"]);
            assert_eq!(names(&open(tmp.path())), ["..", "src"]);
        }

        #[test]
        fn lists_vscode_workspaces_after_the_folders() {
            let tmp = tree(&["src", "z"]);
            for file in ["a.code-workspace", "notes.json"] {
                std::fs::write(tmp.path().join(file), "{}").expect("write file");
            }
            assert_eq!(names(&open(tmp.path())), ["..", "src", "z", "a.code-workspace"]);
        }

        #[test]
        fn root_has_no_parent() {
            assert!(open(Path::new("/")).items().iter().all(|item| item.name != ".."));
        }

        #[test]
        fn shows_the_branch_of_repos() {
            let tmp = TempDir::new();
            let repo = git_repo(&[]);
            let link = tmp.path().join("repo");
            std::fs::rename(repo.path(), &link).expect("move repo");
            std::fs::create_dir(tmp.path().join("plain")).expect("create folder");

            let picker = open(tmp.path());

            let branches: Vec<_> = picker.items().iter().map(|item| item.branch.as_deref()).collect();
            assert_eq!(branches, [None, None, Some("main")]);
        }

        #[test]
        fn subfolders_of_a_repo_show_no_branch() {
            let repo = git_repo(&[("src/main.rs", "")]);
            assert!(open(repo.path()).items().iter().all(|item| item.branch.is_none()));
        }
    }

    mod workspaces {
        use super::*;

        fn with_workspace() -> (TempDir, Picker) {
            let tmp = tree(&["src"]);
            std::fs::write(tmp.path().join("w.code-workspace"), "{}").expect("write file");
            let picker = open(tmp.path());
            (tmp, picker)
        }

        #[test]
        fn choosing_one_returns_it_and_stays() {
            let (tmp, mut picker) = with_workspace();

            let chosen = picker.choose(2);

            assert_eq!((chosen, picker.dir()), (Some(tmp.path().join("w.code-workspace")), tmp.path()));
        }

        #[test]
        fn choosing_a_folder_goes_into_it() {
            let (tmp, mut picker) = with_workspace();

            let chosen = picker.choose(1);

            assert_eq!((chosen, picker.dir()), (None, tmp.path().join("src").as_path()));
        }

        #[test]
        fn going_into_one_does_nothing() {
            let (tmp, mut picker) = with_workspace();

            picker.enter(2);

            assert_eq!(picker.dir(), tmp.path());
        }

        #[test]
        fn filtering_keeps_them_after_the_folders() {
            let tmp = tree(&["data"]);
            std::fs::write(tmp.path().join("a.code-workspace"), "{}").expect("write file");
            let mut picker = open(tmp.path());

            picker.push('a');

            assert_eq!(names(&picker), ["data", "a.code-workspace"]);
        }

        #[test]
        fn submitting_the_selected_one_returns_it() {
            let (tmp, mut picker) = with_workspace();
            type_text(&mut picker, "w.");

            assert_eq!(picker.submit(), Some(tmp.path().join("w.code-workspace")));
        }
    }

    mod filter {
        use super::*;

        #[rstest]
        #[case::exact_and_prefix_first("web", &["aweb", "web-old", "web"], &["web", "web-old", "aweb"])]
        #[case::case_insensitive("SRC", &["src", "other"], &["src"])]
        #[case::hidden_with_a_dot(".c", &[".config", "c"], &[".config"])]
        #[case::nothing_matches("zzz", &["src"], &[])]
        fn ranks_exact_then_prefix_then_substring(
            #[case] filter: &str,
            #[case] folders: &[&str],
            #[case] expected: &[&str],
        ) {
            let tmp = tree(folders);
            let mut picker = open(tmp.path());

            type_text(&mut picker, filter);

            assert_eq!(names(&picker), expected);
        }

        #[test]
        fn typing_selects_the_first_match() {
            let tmp = tree(&["src"]);
            let mut picker = open(tmp.path());

            picker.push('s');

            assert_eq!(picker.selected(), Some(0));
        }

        #[test]
        fn backspace_deletes_from_the_filter() {
            let tmp = tree(&["src"]);
            let mut picker = open(tmp.path());
            type_text(&mut picker, "sr");

            picker.pop();

            assert_eq!((picker.filter(), picker.dir()), ("s", tmp.path()));
        }

        #[test]
        fn clearing_the_filter_clears_the_selection() {
            let tmp = tree(&["src"]);
            let mut picker = open(tmp.path());
            picker.push('s');

            picker.pop();

            assert_eq!(picker.selected(), None);
        }
    }

    mod navigation {
        use super::*;

        #[test]
        fn enter_goes_into_a_folder() {
            let tmp = tree(&["src/inner"]);
            let mut picker = open(tmp.path());

            picker.enter(1);

            assert_eq!((picker.dir(), names(&picker)), (tmp.path().join("src").as_path(), vec!["..", "inner"]));
        }

        #[test]
        fn the_parent_item_goes_up() {
            let tmp = tree(&["src"]);
            let mut picker = open(&tmp.path().join("src"));

            picker.enter(0);

            assert_eq!(picker.dir(), tmp.path());
        }

        #[test]
        fn backspace_with_no_filter_goes_up() {
            let tmp = tree(&["src"]);
            let mut picker = open(&tmp.path().join("src"));

            picker.pop();

            assert_eq!(picker.dir(), tmp.path());
        }

        #[test]
        fn typing_a_path_walks_it() {
            let tmp = tree(&["projects/cornercase", "projects/cornercase-web"]);
            let mut picker = open(Path::new("/"));

            type_text(&mut picker, &format!("{}/projects/cornercase/", tmp.path().display()));

            assert_eq!(picker.dir(), tmp.path().join("projects/cornercase"));
        }

        #[test]
        fn tilde_goes_home() {
            let (home, other) = (TempDir::new(), TempDir::new());
            let mut picker = Picker::open(other.path(), Some(home.path())).expect("open picker");

            picker.push('~');

            assert_eq!(picker.dir(), home.path());
        }

        #[test]
        fn an_unreadable_folder_keeps_the_picker_where_it_was() {
            let tmp = tree(&["src"]);
            let mut picker = open(tmp.path());
            std::fs::remove_dir(tmp.path().join("src")).expect("remove folder");

            picker.enter(1);

            assert_eq!((picker.dir(), picker.error().is_some()), (tmp.path(), true));
        }
    }

    mod submit {
        use super::*;

        #[test]
        fn opens_the_current_folder_when_nothing_is_selected() {
            let tmp = tree(&["src"]);
            assert_eq!(open(tmp.path()).submit(), Some(tmp.path().to_path_buf()));
        }

        #[test]
        fn goes_into_the_selected_folder() {
            let tmp = tree(&["src"]);
            let mut picker = open(tmp.path());
            picker.push('s');

            let opened = picker.submit();

            assert_eq!((opened, picker.dir()), (None, tmp.path().join("src").as_path()));
        }

        #[test]
        fn does_nothing_when_the_filter_matches_nothing() {
            let tmp = tree(&["src"]);
            let mut picker = open(tmp.path());
            picker.push('z');

            assert_eq!((picker.submit(), picker.dir()), (None, tmp.path()));
        }
    }

    mod scrolling {
        use super::*;

        fn many() -> TempDir {
            tree(&["a", "b", "c", "d", "e", "f"])
        }

        #[test]
        fn moving_down_from_nothing_selects_the_first() {
            let tmp = many();
            let mut picker = open(tmp.path());

            picker.move_selection(1, 3);

            assert_eq!(picker.selected(), Some(0));
        }

        #[test]
        fn moving_up_from_nothing_selects_the_last() {
            let tmp = many();
            let mut picker = open(tmp.path());

            picker.move_selection(-1, 3);

            assert_eq!((picker.selected(), picker.scroll()), (Some(6), 4));
        }

        #[test]
        fn moving_past_the_last_visible_row_scrolls() {
            let tmp = many();
            let mut picker = open(tmp.path());
            for _ in 0..4 {
                picker.move_selection(1, 3);
            }

            assert_eq!((picker.selected(), picker.scroll()), (Some(3), 1));
        }

        #[test]
        fn the_wheel_stops_at_the_end() {
            let tmp = many();
            let mut picker = open(tmp.path());

            picker.scroll_by(100, 3);

            assert_eq!(picker.scroll(), 4);
        }

        #[test]
        fn the_wheel_stops_at_the_top() {
            let tmp = many();
            let mut picker = open(tmp.path());

            picker.scroll_by(-3, 3);

            assert_eq!(picker.scroll(), 0);
        }
    }
}
