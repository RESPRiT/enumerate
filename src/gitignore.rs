//! Auto-add `.enumerate/` to the local `.git/info/exclude` when the TUI opens
//! a file inside a `.enumerate/` directory inside a git repo. `.git/info/exclude`
//! is git's per-clone ignore file — never tracked, never committed — so the
//! auto-ignore doesn't pollute the shared repository state. Best-effort: any
//! failure is swallowed by the caller.
//!
//! If the project's tracked `.gitignore` already excludes `.enumerate/` (or
//! carries the opt-out marker), the binary respects that and writes nothing.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

const ENTRY: &str = ".enumerate/";
const HEADER: &str = "# enumerate decision docs";

pub fn ensure_enumerate_ignored(file: &Path) -> Result<()> {
    let Ok(abs) = file.canonicalize() else {
        return Ok(());
    };
    let Some(enumerate_dir) = find_enumerate_ancestor(&abs) else {
        return Ok(());
    };
    let Some(repo_root) = find_git_root(&enumerate_dir) else {
        return Ok(());
    };

    // If the tracked .gitignore already excludes .enumerate/ or carries the
    // opt-out marker, respect that and don't write to .git/info/exclude.
    let gitignore_path = repo_root.join(".gitignore");
    if let Ok(existing) = fs::read_to_string(&gitignore_path)
        && already_handled(&existing)
    {
        return Ok(());
    }

    let exclude_dir = repo_root.join(".git").join("info");
    let exclude_path = exclude_dir.join("exclude");

    let existing = match fs::read_to_string(&exclude_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };

    if already_handled(&existing) {
        return Ok(());
    }

    let mut new_content = existing;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    if !new_content.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str(HEADER);
    new_content.push('\n');
    new_content.push_str(ENTRY);
    new_content.push('\n');

    fs::create_dir_all(&exclude_dir)?;
    fs::write(&exclude_path, new_content)?;
    Ok(())
}

fn find_enumerate_ancestor(file: &Path) -> Option<PathBuf> {
    let mut current = file.parent()?.to_path_buf();
    loop {
        if current.file_name().and_then(|n| n.to_str()) == Some(".enumerate") {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// True if the content already contains either the entry or our header
/// comment. The header acts as an opt-out marker: if the user removes the
/// entry but leaves the header, we treat it as "the user has decided" and
/// don't re-add anything.
fn already_handled(content: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed == HEADER {
            return true;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        let normalized = trimmed
            .trim_start_matches('!')
            .trim_start_matches('/')
            .trim_end_matches('/');
        normalized == ".enumerate"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_enumerate_ancestor_basic() {
        let p = PathBuf::from("/repo/.enumerate/2026-04-08-foo.md");
        assert_eq!(
            find_enumerate_ancestor(&p),
            Some(PathBuf::from("/repo/.enumerate"))
        );
    }

    #[test]
    fn finds_nested_enumerate_ancestor() {
        let p = PathBuf::from("/repo/sub/.enumerate/foo.md");
        assert_eq!(
            find_enumerate_ancestor(&p),
            Some(PathBuf::from("/repo/sub/.enumerate"))
        );
    }

    #[test]
    fn no_enumerate_ancestor() {
        let p = PathBuf::from("/repo/docs/foo.md");
        assert_eq!(find_enumerate_ancestor(&p), None);
    }

    #[test]
    fn already_handled_entry_variants() {
        assert!(already_handled(".enumerate/\n"));
        assert!(already_handled(".enumerate\n"));
        assert!(already_handled("/.enumerate/\n"));
        assert!(already_handled("/.enumerate\n"));
        assert!(already_handled("foo\n.enumerate/\nbar\n"));
        assert!(already_handled("  .enumerate/  \n"));
    }

    #[test]
    fn already_handled_header_acts_as_optout() {
        // Header alone means user removed entry but kept marker — opt out.
        assert!(already_handled("target/\n# enumerate decision docs\n"));
    }

    #[test]
    fn not_already_handled() {
        assert!(!already_handled(""));
        assert!(!already_handled("foo\nbar\n"));
        assert!(!already_handled("# .enumerate/\n"));
        assert!(!already_handled(".enumerate-old/\n"));
        assert!(!already_handled("sub/.enumerate/\n"));
    }

    #[test]
    fn ensure_creates_exclude_in_temp_repo() {
        let tmp = tempdir();
        fs::create_dir(tmp.join(".git")).unwrap();
        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("2026-04-08-foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        // The local exclude file gets created with the header + entry.
        let exclude = fs::read_to_string(tmp.join(".git/info/exclude")).unwrap();
        assert!(exclude.contains(".enumerate/"));
        assert!(exclude.contains("# enumerate decision docs"));

        // The tracked .gitignore is untouched.
        assert!(!tmp.join(".gitignore").exists());
    }

    #[test]
    fn ensure_creates_info_dir_if_missing() {
        let tmp = tempdir();
        fs::create_dir(tmp.join(".git")).unwrap();
        // Note: .git/info does not exist yet — the binary must create it.
        assert!(!tmp.join(".git/info").exists());

        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        assert!(tmp.join(".git/info/exclude").exists());
    }

    #[test]
    fn ensure_appends_to_existing_exclude() {
        let tmp = tempdir();
        fs::create_dir_all(tmp.join(".git/info")).unwrap();
        fs::write(
            tmp.join(".git/info/exclude"),
            "# git ls-files --others --exclude-from=.git/info/exclude\n*~\n",
        )
        .unwrap();
        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        let exclude = fs::read_to_string(tmp.join(".git/info/exclude")).unwrap();
        assert!(exclude.starts_with("# git ls-files"));
        assert!(exclude.contains("*~"));
        assert!(exclude.contains(".enumerate/"));
        assert!(exclude.contains("# enumerate decision docs"));
    }

    #[test]
    fn ensure_is_noop_when_exclude_already_has_entry() {
        let tmp = tempdir();
        fs::create_dir_all(tmp.join(".git/info")).unwrap();
        let original = "*~\n.enumerate/\n";
        fs::write(tmp.join(".git/info/exclude"), original).unwrap();
        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        let exclude = fs::read_to_string(tmp.join(".git/info/exclude")).unwrap();
        assert_eq!(exclude, original);
    }

    #[test]
    fn ensure_respects_gitignore_existing_entry() {
        // If a project commits .enumerate/ to its tracked .gitignore, the
        // binary respects that and doesn't also write to .git/info/exclude.
        let tmp = tempdir();
        fs::create_dir(tmp.join(".git")).unwrap();
        fs::write(tmp.join(".gitignore"), "target/\n.enumerate/\n").unwrap();
        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        // .git/info/exclude was not touched.
        assert!(!tmp.join(".git/info/exclude").exists());
    }

    #[test]
    fn ensure_respects_gitignore_header_optout() {
        // Header in .gitignore without the entry = explicit opt-out.
        let tmp = tempdir();
        fs::create_dir(tmp.join(".git")).unwrap();
        fs::write(tmp.join(".gitignore"), "target/\n# enumerate decision docs\n").unwrap();
        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        assert!(!tmp.join(".git/info/exclude").exists());
    }

    #[test]
    fn ensure_respects_exclude_header_optout() {
        // Header in .git/info/exclude without the entry = explicit opt-out.
        let tmp = tempdir();
        fs::create_dir_all(tmp.join(".git/info")).unwrap();
        let original = "*~\n# enumerate decision docs\n";
        fs::write(tmp.join(".git/info/exclude"), original).unwrap();
        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        let exclude = fs::read_to_string(tmp.join(".git/info/exclude")).unwrap();
        assert_eq!(exclude, original);
    }

    #[test]
    fn ensure_noop_when_no_git_repo() {
        let tmp = tempdir();
        let dir = tmp.join(".enumerate");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        assert!(!tmp.join(".gitignore").exists());
        assert!(!tmp.join(".git/info/exclude").exists());
    }

    #[test]
    fn ensure_noop_when_file_not_in_enumerate() {
        let tmp = tempdir();
        fs::create_dir(tmp.join(".git")).unwrap();
        let file = tmp.join("foo.md");
        fs::write(&file, "x").unwrap();

        ensure_enumerate_ignored(&file).unwrap();

        assert!(!tmp.join(".gitignore").exists());
        assert!(!tmp.join(".git/info/exclude").exists());
    }

    /// Minimal tempdir helper that picks a unique path under std::env::temp_dir.
    /// Cleaned up via Drop.
    fn tempdir() -> TempDir {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "enumerate-gitignore-test-{}-{}",
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        TempDir { path }
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn join(&self, p: &str) -> PathBuf {
            self.path.join(p)
        }
    }

    impl std::ops::Deref for TempDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
