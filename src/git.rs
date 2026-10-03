use std::path::{Path, PathBuf};

const SHORT_SHA_LEN: usize = 7;

pub fn branch(dir: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git_dir(dir)?.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref:") {
        let reference = reference.trim();
        return Some(reference.strip_prefix("refs/heads/").unwrap_or(reference).to_string());
    }
    head.get(..SHORT_SHA_LEN).map(str::to_string)
}

pub fn main_worktree(dir: &Path) -> Option<PathBuf> {
    let root = dir.ancestors().find(|d| d.join(".git").exists())?;
    root.join(".git").is_dir().then(|| root.to_path_buf())
}

pub fn is_repo_root(dir: &Path) -> bool {
    dir.join(".git").is_dir()
}

pub fn linked_worktrees(repo: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(repo.join(".git").join("worktrees")) else { return Vec::new() };
    let mut worktrees: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("gitdir")).ok())
        .filter_map(|gitdir| Path::new(gitdir.trim()).parent().map(Path::to_path_buf))
        .filter(|path| path.is_dir())
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect();
    worktrees.sort();
    worktrees
}

fn git_dir(dir: &Path) -> Option<PathBuf> {
    dir.ancestors().find_map(|d| {
        let dot_git = d.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        let link = std::fs::read_to_string(&dot_git).ok()?;
        let target = link.trim().strip_prefix("gitdir:")?.trim();
        Some(d.join(target))
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_util::TempDir;

    fn repo_with_head(head: &str) -> TempDir {
        let tmp = TempDir::new();
        std::fs::create_dir(tmp.path().join(".git")).expect("create .git");
        std::fs::write(tmp.path().join(".git/HEAD"), head).expect("write HEAD");
        tmp
    }

    mod main_worktree {
        use super::*;

        #[test]
        fn is_the_folder_holding_the_git_dir() {
            let repo = repo_with_head("ref: refs/heads/main\n");
            let sub = repo.path().join("src");
            std::fs::create_dir(&sub).expect("create subfolder");
            assert_eq!(main_worktree(&sub), Some(repo.path().to_path_buf()));
        }

        #[test]
        fn is_none_inside_a_linked_worktree() {
            let tmp = TempDir::new();
            std::fs::write(tmp.path().join(".git"), "gitdir: /repo/.git/worktrees/wt\n").expect("write .git file");
            assert_eq!(main_worktree(tmp.path()), None);
        }

        #[test]
        fn is_none_outside_a_repo() {
            let tmp = TempDir::new();
            assert_eq!(main_worktree(tmp.path()), None);
        }
    }

    mod linked_worktrees {
        use super::*;
        use crate::test_util::{git, git_repo};

        #[test]
        fn lists_the_checkouts_made_with_git() {
            let repo = git_repo(&[]);
            let (a, b) = (TempDir::new(), TempDir::new());
            let (a_path, b_path) = (a.path().join("a"), b.path().join("b"));
            git(repo.path(), &["worktree", "add", "--quiet", "-b", "b", &b_path.display().to_string()]);
            git(repo.path(), &["worktree", "add", "--quiet", "-b", "a", &a_path.display().to_string()]);

            let mut expected = vec![a_path.canonicalize().expect("a"), b_path.canonicalize().expect("b")];
            expected.sort();
            assert_eq!(linked_worktrees(repo.path()), expected);
        }

        #[test]
        fn leaves_out_checkouts_whose_folder_is_gone() {
            let repo = git_repo(&[]);
            let tmp = TempDir::new();
            let path = tmp.path().join("gone");
            git(repo.path(), &["worktree", "add", "--quiet", "-b", "gone", &path.display().to_string()]);
            std::fs::remove_dir_all(&path).expect("remove worktree folder");

            assert_eq!(linked_worktrees(repo.path()), Vec::<PathBuf>::new());
        }

        #[test]
        fn is_empty_without_worktrees() {
            let repo = git_repo(&[]);
            assert_eq!(linked_worktrees(repo.path()), Vec::<PathBuf>::new());
        }
    }

    mod branch {
        use super::*;

        #[rstest]
        #[case::simple("ref: refs/heads/main\n", "main")]
        #[case::with_slashes("ref: refs/heads/feature/login\n", "feature/login")]
        #[case::detached_shows_short_sha("3f2a9c1d8e7b6a5f4e3d2c1b0a9f8e7d6c5b4a39\n", "3f2a9c1")]
        fn reads_head(#[case] head: &str, #[case] expected: &str) {
            let repo = repo_with_head(head);
            assert_eq!(branch(repo.path()).as_deref(), Some(expected));
        }

        #[test]
        fn is_found_from_a_subfolder() {
            let repo = repo_with_head("ref: refs/heads/main\n");
            let sub = repo.path().join("src/deep");
            std::fs::create_dir_all(&sub).expect("create subfolder");
            assert_eq!(branch(&sub).as_deref(), Some("main"));
        }

        #[test]
        fn follows_a_gitdir_file() {
            let repo = repo_with_head("ref: refs/heads/main\n");
            let worktrees = repo.path().join(".git/worktrees/wt");
            std::fs::create_dir_all(&worktrees).expect("create worktree gitdir");
            std::fs::write(worktrees.join("HEAD"), "ref: refs/heads/hotfix\n").expect("write HEAD");
            let wt = repo.path().join("wt");
            std::fs::create_dir(&wt).expect("create worktree");
            std::fs::write(wt.join(".git"), "gitdir: ../.git/worktrees/wt\n").expect("write .git file");
            assert_eq!(branch(&wt).as_deref(), Some("hotfix"));
        }

        #[test]
        fn is_none_outside_a_repo() {
            let tmp = TempDir::new();
            assert_eq!(branch(tmp.path()), None);
        }
    }
}
