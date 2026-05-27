use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatus {
    pub initialized: bool,
    pub branch: String,
    pub dirty: bool,
    pub raw_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitChangedFile {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub diff: String,
}

pub fn status(repo: impl AsRef<Path>) -> Result<GitStatus, String> {
    let repo = repo.as_ref();
    if !repo.join(".git").exists() {
        return Ok(GitStatus {
            initialized: false,
            branch: String::new(),
            dirty: false,
            raw_status: String::new(),
        });
    }

    let branch = git(repo, &["branch", "--show-current"])?;
    let raw_status = git(repo, &["status", "--short", "--branch"])?;
    let dirty = raw_status.lines().any(|line| !line.starts_with("##"));

    Ok(GitStatus {
        initialized: true,
        branch: branch.trim().to_string(),
        dirty,
        raw_status,
    })
}

pub fn init_main(repo: impl AsRef<Path>) -> Result<(), String> {
    let repo = repo.as_ref();
    fs::create_dir_all(repo).map_err(|error| error.to_string())?;
    git(repo, &["init", "-b", "main"])?;
    Ok(())
}

pub fn origin_url(repo: impl AsRef<Path>) -> Result<Option<String>, String> {
    let repo = repo.as_ref();
    match git(repo, &["remote", "get-url", "origin"]) {
        Ok(url) => Ok(Some(url.trim().to_string()).filter(|value| !value.is_empty())),
        Err(error) if error.contains("No such remote") => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn set_origin_url(repo: impl AsRef<Path>, remote_url: &str) -> Result<(), String> {
    let repo = repo.as_ref();
    if origin_url(repo)?.is_some() {
        git(repo, &["remote", "set-url", "origin", remote_url])?;
    } else {
        git(repo, &["remote", "add", "origin", remote_url])?;
    }
    Ok(())
}

pub fn add_all(repo: impl AsRef<Path>) -> Result<(), String> {
    git(repo.as_ref(), &["add", "."])?;
    Ok(())
}

pub fn add_paths(repo: impl AsRef<Path>, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("Select at least one file to commit.".to_string());
    }

    let mut args = vec!["add".to_string(), "--".to_string()];
    args.extend(paths.iter().cloned());
    git_owned(repo.as_ref(), &args)?;
    Ok(())
}

pub fn staged_changes(repo: impl AsRef<Path>) -> Result<bool, String> {
    let status = git(repo.as_ref(), &["diff", "--cached", "--name-only"])?;
    Ok(!status.trim().is_empty())
}

pub fn commit(repo: impl AsRef<Path>, message: &str) -> Result<String, String> {
    let repo = repo.as_ref();
    git_with_config(repo, &["commit", "-m", message])?;
    Ok(git(repo, &["rev-parse", "HEAD"])?.trim().to_string())
}

pub fn push_origin_main(repo: impl AsRef<Path>, set_upstream: bool) -> Result<(), String> {
    let args: &[&str] = if set_upstream {
        &["push", "-u", "origin", "main"]
    } else {
        &["push", "origin", "main"]
    };
    git(repo.as_ref(), args)?;
    Ok(())
}

pub fn changed_files(repo: impl AsRef<Path>) -> Result<Vec<GitChangedFile>, String> {
    let output = git(
        repo.as_ref(),
        &["status", "--porcelain=v1", "--untracked-files=all", "-z"],
    )?;
    let mut entries = output.split('\0').filter(|entry| !entry.is_empty());
    let mut files = Vec::new();

    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }

        let status = entry[0..2].to_string();
        let mut path = entry[3..].to_string();
        if status.starts_with('R') || status.starts_with('C') {
            if let Some(new_path) = entries.next() {
                path = new_path.to_string();
            }
        }

        files.push(GitChangedFile { path, status });
    }

    Ok(files)
}

pub fn has_head(repo: impl AsRef<Path>) -> bool {
    git(repo.as_ref(), &["rev-parse", "--verify", "HEAD"]).is_ok()
}

pub fn diff_head_path(repo: impl AsRef<Path>, path: &str) -> Result<String, String> {
    git_owned(
        repo.as_ref(),
        &[
            "diff".to_string(),
            "--no-ext-diff".to_string(),
            "HEAD".to_string(),
            "--".to_string(),
            path.to_string(),
        ],
    )
}

pub fn ls_remote(repo_url: &str, reference: &str) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .arg("ls-remote")
        .arg(repo_url)
        .arg(reference)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(str::to_string))
}

pub fn fetch_ref_path(
    repo_url: &str,
    reference: &str,
    path: &str,
    checkout_root: impl AsRef<Path>,
) -> Result<String, String> {
    let checkout_root = checkout_root.as_ref();
    fs::create_dir_all(checkout_root).map_err(|error| error.to_string())?;
    git(checkout_root, &["init", "-b", "main"])?;
    git(checkout_root, &["remote", "add", "origin", repo_url])?;
    git(
        checkout_root,
        &["fetch", "--depth", "1", "origin", reference],
    )?;
    let sha = git(checkout_root, &["rev-parse", "FETCH_HEAD"])?
        .trim()
        .to_string();
    git_owned(
        checkout_root,
        &[
            "checkout".to_string(),
            "FETCH_HEAD".to_string(),
            "--".to_string(),
            path.to_string(),
        ],
    )?;
    Ok(sha)
}

pub fn diff_no_index_tree(
    old_root: impl AsRef<Path>,
    new_root: impl AsRef<Path>,
) -> Result<Vec<GitDiffFile>, String> {
    let old_root = old_root.as_ref();
    let new_root = new_root.as_ref();
    let old_root_text = old_root.to_str().ok_or("Old path is not valid UTF-8.")?;
    let new_root_text = new_root.to_str().ok_or("New path is not valid UTF-8.")?;

    let name_status = git_diff_no_index(&[
        "--no-index",
        "--name-status",
        "-M",
        old_root_text,
        new_root_text,
    ])?;
    let unified = git_diff_no_index(&["--no-index", "-M", "--", old_root_text, new_root_text])?;
    Ok(parse_no_index_files(
        &name_status,
        &unified,
        old_root,
        new_root,
    ))
}

fn git_diff_no_index(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("diff")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;

    if output.status.success() || output.status.code() == Some(1) {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

fn parse_no_index_files(
    name_status: &str,
    unified: &str,
    old_root: &Path,
    new_root: &Path,
) -> Vec<GitDiffFile> {
    let mut sections_by_path = HashMap::new();
    for section in split_diff_sections(unified) {
        if let Some((old_path, new_path)) = diff_section_paths(&section, old_root, new_root) {
            let normalized = normalize_diff_section(&section, &old_path, &new_path);
            let key = if new_path.is_empty() {
                old_path.clone()
            } else {
                new_path.clone()
            };
            sections_by_path.insert(key.clone(), normalized.clone());
            if !old_path.is_empty() && old_path != key {
                sections_by_path.insert(old_path, normalized);
            }
        }
    }

    name_status
        .lines()
        .filter_map(|line| parse_name_status_line(line, old_root, new_root, &sections_by_path))
        .collect()
}

fn parse_name_status_line(
    line: &str,
    old_root: &Path,
    new_root: &Path,
    sections_by_path: &HashMap<String, String>,
) -> Option<GitDiffFile> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 2 {
        return None;
    }

    let status_code = parts[0];
    let status = status_code.chars().next()?.to_string();
    let old_path = if matches!(status.as_str(), "R" | "C") && parts.len() >= 3 {
        Some(normalize_no_index_path(parts[1], old_root, new_root))
    } else {
        None
    };
    let path_source = if matches!(status.as_str(), "R" | "C") && parts.len() >= 3 {
        parts[2]
    } else {
        parts[1]
    };
    let path = normalize_no_index_path(path_source, old_root, new_root);
    let diff = sections_by_path.get(&path).cloned().unwrap_or_default();

    Some(GitDiffFile {
        path,
        old_path,
        status,
        diff,
    })
}

fn split_diff_sections(unified: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current = Vec::new();

    for line in unified.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            sections.push(current.join("\n") + "\n");
            current.clear();
        }
        current.push(line.to_string());
    }

    if !current.is_empty() {
        sections.push(current.join("\n") + "\n");
    }

    sections
}

fn diff_section_paths(section: &str, old_root: &Path, new_root: &Path) -> Option<(String, String)> {
    let header = section.lines().next()?;
    let rest = header.strip_prefix("diff --git ")?;
    let (old_path, new_path) = rest.split_once(" b/")?;
    Some((
        normalize_no_index_path(
            old_path.strip_prefix("a/").unwrap_or(old_path),
            old_root,
            new_root,
        ),
        normalize_no_index_path(new_path, old_root, new_root),
    ))
}

fn normalize_diff_section(section: &str, old_path: &str, new_path: &str) -> String {
    let mut normalized = String::new();
    for line in section.lines() {
        if line.starts_with("diff --git ") {
            normalized.push_str(&format!("diff --git a/{old_path} b/{new_path}\n"));
        } else if line.starts_with("--- /dev/null") || line.starts_with("+++ /dev/null") {
            normalized.push_str(line);
            normalized.push('\n');
        } else if line.starts_with("--- ") {
            normalized.push_str(&format!("--- a/{old_path}\n"));
        } else if line.starts_with("+++ ") {
            normalized.push_str(&format!("+++ b/{new_path}\n"));
        } else {
            normalized.push_str(line);
            normalized.push('\n');
        }
    }
    normalized
}

fn normalize_no_index_path(path: &str, old_root: &Path, new_root: &Path) -> String {
    let value = path
        .trim()
        .trim_matches('"')
        .strip_prefix("a/")
        .or_else(|| path.trim().trim_matches('"').strip_prefix("b/"))
        .unwrap_or_else(|| path.trim().trim_matches('"'));
    if value == "/dev/null" {
        return String::new();
    }

    for root in [old_root, new_root] {
        let root_text = root.to_string_lossy();
        if let Some(stripped) = strip_root_prefix(value, &root_text) {
            return stripped;
        }

        let without_leading_slash = root_text.trim_start_matches('/');
        if let Some(stripped) = strip_root_prefix(value, without_leading_slash) {
            return stripped;
        }
    }

    value.trim_start_matches('/').to_string()
}

fn strip_root_prefix(value: &str, root: &str) -> Option<String> {
    if value == root {
        return Some(String::new());
    }
    value
        .strip_prefix(root)
        .map(|path| path.trim_start_matches('/').to_string())
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_owned(repo: &Path, args: &[String]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_with_config(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("-c")
        .arg("user.name=SkillBox")
        .arg("-c")
        .arg("user.email=skillbox@example.invalid")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn init_add_commit_and_status_report_clean_repo() {
        let temp = temp_dir("skillbox-git-clean");
        write_file(&temp.join("demo.txt"), "demo");

        init_main(&temp).unwrap();
        add_all(&temp).unwrap();
        let sha = commit(&temp, "Initial sync").unwrap();
        let status = status(&temp).unwrap();

        assert!(!sha.is_empty());
        assert!(status.initialized);
        assert_eq!(status.branch, "main");
        assert!(!status.dirty);
    }

    #[test]
    fn remote_url_can_be_added_and_updated() {
        let temp = temp_dir("skillbox-git-remote");
        init_main(&temp).unwrap();

        set_origin_url(&temp, "https://example.com/one.git").unwrap();
        assert_eq!(
            origin_url(&temp).unwrap(),
            Some("https://example.com/one.git".to_string())
        );

        set_origin_url(&temp, "https://example.com/two.git").unwrap();
        assert_eq!(
            origin_url(&temp).unwrap(),
            Some("https://example.com/two.git".to_string())
        );
    }

    #[test]
    fn snapshot_fetch_ref_path_checks_out_only_requested_path() {
        let remote = bare_remote_with_skill("git-snapshot-origin");
        let temp = temp_dir("git-snapshot-work");
        let checkout = temp.join("checkout");

        let sha =
            fetch_ref_path(remote.to_str().unwrap(), "main", "skills/demo", &checkout).unwrap();

        assert!(!sha.is_empty());
        assert!(checkout.join("skills/demo/SKILL.md").exists());
        assert!(!checkout.join("README.md").exists());
    }

    #[test]
    fn snapshot_diff_no_index_tree_reports_changed_files() {
        let temp = temp_dir("git-diff-no-index");
        let old_root = temp.join("old");
        let new_root = temp.join("new");
        fs::create_dir_all(&old_root).unwrap();
        fs::create_dir_all(&new_root).unwrap();
        fs::write(old_root.join("SKILL.md"), "name: demo\n").unwrap();
        fs::write(new_root.join("SKILL.md"), "name: demo\nversion: 2\n").unwrap();
        fs::write(new_root.join("extra.txt"), "extra\n").unwrap();

        let files = diff_no_index_tree(&old_root, &new_root).unwrap();

        assert!(files
            .iter()
            .any(|file| file.path == "SKILL.md" && file.status == "M"));
        assert!(files
            .iter()
            .any(|file| file.path == "extra.txt" && file.status == "A"));
        let skill_diff = files
            .iter()
            .find(|file| file.path == "SKILL.md")
            .map(|file| file.diff.as_str())
            .unwrap_or("");
        assert!(skill_diff.starts_with("diff --git a/SKILL.md b/SKILL.md"));
        assert!(skill_diff.contains("--- a/SKILL.md"));
        assert!(skill_diff.contains("+++ b/SKILL.md"));
        assert!(skill_diff.contains("+version: 2"));
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_file(path: &Path, content: &str) {
        fs::write(path, content).unwrap();
    }

    fn bare_remote_with_skill(label: &str) -> PathBuf {
        let remote = temp_dir(label).join("remote.git");
        Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()
            .unwrap();
        let work = temp_dir(&format!("{label}-work"));
        Command::new("git")
            .args(["init", "-b", "main"])
            .arg(&work)
            .output()
            .unwrap();
        fs::create_dir_all(work.join("skills/demo")).unwrap();
        fs::write(work.join("README.md"), "root\n").unwrap();
        fs::write(
            work.join("skills/demo/SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\n",
        )
        .unwrap();
        git(&work, &["add", "."]).unwrap();
        git_with_config(&work, &["commit", "-m", "Initial skill"]).unwrap();
        git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )
        .unwrap();
        git(&work, &["push", "-u", "origin", "main"]).unwrap();
        remote
    }
}
