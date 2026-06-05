use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_LS_REMOTE_TIMEOUT: Duration = Duration::from_secs(30);
const FETCH_REF_TIMEOUT: Duration = Duration::from_secs(30);
const PUSH_TIMEOUT: Duration = Duration::from_secs(30);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLogEntry {
    pub sha: String,
    pub timestamp: String,
    pub subject: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GitService;

impl GitService {
    pub fn new() -> Self {
        Self
    }

    pub fn status(&self, repo: impl AsRef<Path>) -> Result<GitStatus, String> {
        let repo = repo.as_ref();
        if !repo.join(".git").exists() {
            return Ok(GitStatus {
                initialized: false,
                branch: String::new(),
                dirty: false,
                raw_status: String::new(),
            });
        }

        let branch = self.run(repo, &["branch", "--show-current"])?;
        let raw_status = self.run(repo, &["status", "--short", "--branch"])?;
        let dirty = raw_status.lines().any(|line| !line.starts_with("##"));

        Ok(GitStatus {
            initialized: true,
            branch: branch.trim().to_string(),
            dirty,
            raw_status,
        })
    }

    pub fn init_main(&self, repo: impl AsRef<Path>) -> Result<(), String> {
        let repo = repo.as_ref();
        fs::create_dir_all(repo).map_err(|error| error.to_string())?;
        self.run(repo, &["init", "-b", "main"])?;
        Ok(())
    }

    pub fn origin_url(&self, repo: impl AsRef<Path>) -> Result<Option<String>, String> {
        let repo = repo.as_ref();
        match self.run(repo, &["remote", "get-url", "origin"]) {
            Ok(url) => Ok(Some(url.trim().to_string()).filter(|value| !value.is_empty())),
            Err(error) if error.contains("No such remote") => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn set_origin_url(&self, repo: impl AsRef<Path>, remote_url: &str) -> Result<(), String> {
        let repo = repo.as_ref();
        if self.origin_url(repo)?.is_some() {
            self.run(repo, &["remote", "set-url", "origin", remote_url])?;
        } else {
            self.run(repo, &["remote", "add", "origin", remote_url])?;
        }
        Ok(())
    }

    pub fn add_all(&self, repo: impl AsRef<Path>) -> Result<(), String> {
        self.run(repo.as_ref(), &["add", "."])?;
        Ok(())
    }

    pub fn add_paths(&self, repo: impl AsRef<Path>, paths: &[String]) -> Result<(), String> {
        if paths.is_empty() {
            return Err("Select at least one file to commit.".to_string());
        }

        let mut args = vec!["add".to_string(), "--".to_string()];
        args.extend(paths.iter().cloned());
        self.run_owned(repo.as_ref(), &args)?;
        Ok(())
    }

    pub fn staged_changes(&self, repo: impl AsRef<Path>) -> Result<bool, String> {
        let status = self.run(repo.as_ref(), &["diff", "--cached", "--name-only"])?;
        Ok(!status.trim().is_empty())
    }

    pub fn commit(&self, repo: impl AsRef<Path>, message: &str) -> Result<String, String> {
        let repo = repo.as_ref();
        self.run_with_config(repo, &["commit", "-m", message])?;
        Ok(self.run(repo, &["rev-parse", "HEAD"])?.trim().to_string())
    }

    pub fn push_origin_main(
        &self,
        repo: impl AsRef<Path>,
        set_upstream: bool,
    ) -> Result<(), String> {
        let args: &[&str] = if set_upstream {
            &["push", "-u", "origin", "main"]
        } else {
            &["push", "origin", "main"]
        };
        self.run_network(repo.as_ref(), args, PUSH_TIMEOUT, "git push")?;
        Ok(())
    }

    pub fn changed_files(&self, repo: impl AsRef<Path>) -> Result<Vec<GitChangedFile>, String> {
        let output = self.run(
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

    pub fn has_head(&self, repo: impl AsRef<Path>) -> bool {
        self.run(repo.as_ref(), &["rev-parse", "--verify", "HEAD"])
            .is_ok()
    }

    pub fn diff_head_path(&self, repo: impl AsRef<Path>, path: &str) -> Result<String, String> {
        self.run_owned(
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

    pub fn log_path(
        &self,
        repo: impl AsRef<Path>,
        path: &str,
        limit: usize,
    ) -> Result<Vec<GitLogEntry>, String> {
        let repo = repo.as_ref();
        if !self.has_head(repo) {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, 100).to_string();
        let output = self.run_owned(
            repo,
            &[
                "log".to_string(),
                format!("-n{limit}"),
                "--format=%H%x1f%ct%x1f%s%x1e".to_string(),
                "--".to_string(),
                path.to_string(),
            ],
        )?;

        Ok(output.split('\x1e').filter_map(parse_log_entry).collect())
    }

    pub fn ls_remote(&self, repo_url: &str, reference: &str) -> Result<Option<String>, String> {
        self.ls_remote_with_timeout(repo_url, reference, DEFAULT_LS_REMOTE_TIMEOUT)
    }

    pub fn ls_remote_with_timeout(
        &self,
        repo_url: &str,
        reference: &str,
        timeout: Duration,
    ) -> Result<Option<String>, String> {
        validate_git_remote_arg(repo_url)?;
        validate_git_reference_arg(reference)?;
        let mut command = Command::new("git");
        command
            .arg("ls-remote")
            .arg("--")
            .arg(repo_url)
            .arg(reference)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "true")
            .env("GCM_INTERACTIVE", "never");
        let output = self.command_output_with_timeout(command, timeout, "git ls-remote")?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .map(str::to_string))
    }

    pub fn fetch_ref_path(
        &self,
        repo_url: &str,
        reference: &str,
        path: &str,
        checkout_root: impl AsRef<Path>,
    ) -> Result<String, String> {
        self.fetch_ref_path_with_timeout(
            repo_url,
            reference,
            path,
            checkout_root,
            FETCH_REF_TIMEOUT,
        )
    }

    pub fn fetch_ref_path_with_timeout(
        &self,
        repo_url: &str,
        reference: &str,
        path: &str,
        checkout_root: impl AsRef<Path>,
        timeout: Duration,
    ) -> Result<String, String> {
        validate_git_remote_arg(repo_url)?;
        validate_git_reference_arg(reference)?;
        let checkout_root = checkout_root.as_ref();
        fs::create_dir_all(checkout_root).map_err(|error| error.to_string())?;
        self.run(checkout_root, &["init", "-b", "main"])?;
        self.run(checkout_root, &["remote", "add", "origin", repo_url])?;
        self.run_network(
            checkout_root,
            &["fetch", "--depth", "1", "origin", "--", reference],
            timeout,
            "git fetch",
        )?;
        let sha = self
            .run(checkout_root, &["rev-parse", "FETCH_HEAD"])?
            .trim()
            .to_string();
        self.run_owned(
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
        &self,
        old_root: impl AsRef<Path>,
        new_root: impl AsRef<Path>,
    ) -> Result<Vec<GitDiffFile>, String> {
        let old_root = old_root.as_ref();
        let new_root = new_root.as_ref();
        let old_root_text = old_root.to_str().ok_or("Old path is not valid UTF-8.")?;
        let new_root_text = new_root.to_str().ok_or("New path is not valid UTF-8.")?;

        let name_status = self.run_diff_no_index(&[
            "--no-index",
            "--name-status",
            "-M",
            old_root_text,
            new_root_text,
        ])?;
        let unified =
            self.run_diff_no_index(&["--no-index", "-M", "--", old_root_text, new_root_text])?;
        Ok(parse_no_index_files(
            &name_status,
            &unified,
            old_root,
            new_root,
        ))
    }

    fn run(&self, repo: &Path, args: &[&str]) -> Result<String, String> {
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

    fn run_owned(&self, repo: &Path, args: &[String]) -> Result<String, String> {
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

    fn run_network(
        &self,
        repo: &Path,
        args: &[&str],
        timeout: Duration,
        label: &str,
    ) -> Result<String, String> {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "true")
            .env("GCM_INTERACTIVE", "never");
        let output = self.command_output_with_timeout(command, timeout, label)?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn run_with_config(&self, repo: &Path, args: &[&str]) -> Result<String, String> {
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

    fn run_diff_no_index(&self, args: &[&str]) -> Result<String, String> {
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

    fn command_output_with_timeout(
        &self,
        mut command: Command,
        timeout: Duration,
        label: &str,
    ) -> Result<Output, String> {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        let started_at = Instant::now();

        loop {
            if child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                return child.wait_with_output().map_err(|error| error.to_string());
            }

            if started_at.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{label} timed out after {}",
                    format_duration(timeout)
                ));
            }

            thread::sleep(Duration::from_millis(20));
        }
    }
}

pub fn status(repo: impl AsRef<Path>) -> Result<GitStatus, String> {
    GitService::new().status(repo)
}

pub fn init_main(repo: impl AsRef<Path>) -> Result<(), String> {
    GitService::new().init_main(repo)
}

pub fn origin_url(repo: impl AsRef<Path>) -> Result<Option<String>, String> {
    GitService::new().origin_url(repo)
}

pub fn set_origin_url(repo: impl AsRef<Path>, remote_url: &str) -> Result<(), String> {
    GitService::new().set_origin_url(repo, remote_url)
}

pub fn add_all(repo: impl AsRef<Path>) -> Result<(), String> {
    GitService::new().add_all(repo)
}

pub fn add_paths(repo: impl AsRef<Path>, paths: &[String]) -> Result<(), String> {
    GitService::new().add_paths(repo, paths)
}

pub fn staged_changes(repo: impl AsRef<Path>) -> Result<bool, String> {
    GitService::new().staged_changes(repo)
}

pub fn commit(repo: impl AsRef<Path>, message: &str) -> Result<String, String> {
    GitService::new().commit(repo, message)
}

pub fn push_origin_main(repo: impl AsRef<Path>, set_upstream: bool) -> Result<(), String> {
    GitService::new().push_origin_main(repo, set_upstream)
}

pub fn changed_files(repo: impl AsRef<Path>) -> Result<Vec<GitChangedFile>, String> {
    GitService::new().changed_files(repo)
}

pub fn has_head(repo: impl AsRef<Path>) -> bool {
    GitService::new().has_head(repo)
}

pub fn diff_head_path(repo: impl AsRef<Path>, path: &str) -> Result<String, String> {
    GitService::new().diff_head_path(repo, path)
}

pub fn log_path(
    repo: impl AsRef<Path>,
    path: &str,
    limit: usize,
) -> Result<Vec<GitLogEntry>, String> {
    GitService::new().log_path(repo, path, limit)
}

fn parse_log_entry(entry: &str) -> Option<GitLogEntry> {
    let trimmed = entry.trim_matches(|character| character == '\n' || character == '\r');
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.splitn(3, '\x1f');
    Some(GitLogEntry {
        sha: parts.next()?.to_string(),
        timestamp: parts.next()?.to_string(),
        subject: parts.next().unwrap_or("").to_string(),
    })
}

pub fn ls_remote(repo_url: &str, reference: &str) -> Result<Option<String>, String> {
    GitService::new().ls_remote(repo_url, reference)
}

pub fn ls_remote_with_timeout(
    repo_url: &str,
    reference: &str,
    timeout: Duration,
) -> Result<Option<String>, String> {
    GitService::new().ls_remote_with_timeout(repo_url, reference, timeout)
}

fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis % 1000 == 0 {
        format!("{}s", millis / 1000)
    } else {
        format!("{millis}ms")
    }
}

pub fn fetch_ref_path(
    repo_url: &str,
    reference: &str,
    path: &str,
    checkout_root: impl AsRef<Path>,
) -> Result<String, String> {
    GitService::new().fetch_ref_path(repo_url, reference, path, checkout_root)
}

fn validate_git_remote_arg(repo_url: &str) -> Result<(), String> {
    if repo_url.trim().is_empty() {
        return Err("Git remote URL is required.".to_string());
    }
    if repo_url.trim_start().starts_with('-') {
        return Err("Git remote URL must not start with '-'.".to_string());
    }
    Ok(())
}

fn validate_git_reference_arg(reference: &str) -> Result<(), String> {
    if reference.trim().is_empty() {
        return Err("Git reference is required.".to_string());
    }
    if reference.trim_start().starts_with('-') {
        return Err("Git reference must not start with '-'.".to_string());
    }
    Ok(())
}

pub fn diff_no_index_tree(
    old_root: impl AsRef<Path>,
    new_root: impl AsRef<Path>,
) -> Result<Vec<GitDiffFile>, String> {
    GitService::new().diff_no_index_tree(old_root, new_root)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    fn git_service_runs_structured_local_commands() {
        let git = GitService::new();
        let temp = temp_dir("skillbox-git-service");
        write_file(&temp.join("demo.txt"), "demo");

        git.init_main(&temp).unwrap();
        git.add_all(&temp).unwrap();
        let sha = git.commit(&temp, "Initial sync").unwrap();
        let status = git.status(&temp).unwrap();

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

    #[test]
    fn command_output_times_out_slow_processes() {
        let mut command = Command::new("sleep");
        command.arg("5");

        let error = GitService::new()
            .command_output_with_timeout(
                command,
                std::time::Duration::from_millis(100),
                "slow command",
            )
            .unwrap_err();

        assert!(error.contains("timed out"));
    }

    #[test]
    fn ls_remote_timeout_defaults_to_slow_network_budget() {
        assert_eq!(DEFAULT_LS_REMOTE_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn ls_remote_supports_configured_timeout() {
        let source = include_str!("lib.rs");
        let ls_remote_start = source.find("pub fn ls_remote(").unwrap();
        let fetch_ref_path_start = source.find("pub fn fetch_ref_path").unwrap();
        let ls_remote_source = &source[ls_remote_start..fetch_ref_path_start];

        assert!(ls_remote_source.contains("ls_remote_with_timeout"));
        assert!(ls_remote_source.contains("timeout"));
    }

    #[test]
    fn ls_remote_rejects_option_like_inputs_before_git_runs() {
        let error = ls_remote_with_timeout(
            "--upload-pack=sh",
            "main",
            std::time::Duration::from_millis(100),
        )
        .unwrap_err();

        assert!(error.contains("Git remote URL must not start with '-'"));

        let error = ls_remote_with_timeout(
            "https://github.com/acme/repo.git",
            "--upload-pack=sh",
            std::time::Duration::from_millis(100),
        )
        .unwrap_err();

        assert!(error.contains("Git reference must not start with '-'"));
    }

    #[test]
    fn network_git_commands_delimit_untrusted_arguments() {
        let source = include_str!("lib.rs");
        let ls_remote_start = source.find("pub fn ls_remote(").unwrap();
        let fetch_ref_path_start = source.find("pub fn fetch_ref_path").unwrap();
        let diff_no_index_start = source.find("pub fn diff_no_index_tree").unwrap();
        let ls_remote_source = &source[ls_remote_start..fetch_ref_path_start];
        let fetch_ref_path_source = &source[fetch_ref_path_start..diff_no_index_start];

        assert!(ls_remote_source.contains(".arg(\"--\")"));
        assert!(fetch_ref_path_source.contains("\"--\""));
    }

    #[test]
    fn fetch_ref_path_uses_bounded_noninteractive_fetch() {
        let source = include_str!("lib.rs");
        let fetch_ref_path_start = source.find("pub fn fetch_ref_path").unwrap();
        let diff_no_index_start = source.find("pub fn diff_no_index_tree").unwrap();
        let fetch_ref_path_source = &source[fetch_ref_path_start..diff_no_index_start];
        let run_network_start = source.find("fn run_network").unwrap();
        let run_with_config_start = source.find("fn run_with_config").unwrap();
        let run_network_source = &source[run_network_start..run_with_config_start];

        assert!(fetch_ref_path_source.contains("run_network"));
        assert!(fetch_ref_path_source.contains("FETCH_REF_TIMEOUT"));
        assert!(run_network_source.contains("command_output_with_timeout"));
        assert!(run_network_source.contains("GIT_TERMINAL_PROMPT"));
        assert!(run_network_source.contains("GIT_ASKPASS"));
        assert!(run_network_source.contains("GCM_INTERACTIVE"));
    }

    #[test]
    fn push_origin_main_uses_bounded_noninteractive_push() {
        let source = include_str!("lib.rs");
        let push_start = source.find("pub fn push_origin_main").unwrap();
        let changed_files_start = source.find("pub fn changed_files").unwrap();
        let push_source = &source[push_start..changed_files_start];

        assert!(push_source.contains("run_network"));
        assert!(push_source.contains("PUSH_TIMEOUT"));
        assert!(!push_source.contains("run(repo.as_ref(), args)"));
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
        let git = GitService::new();
        let remote = temp_dir(label).join("remote.git");
        Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()
            .unwrap();
        let work = temp_dir(&format!("{label}-work"));
        git.init_main(&work).unwrap();
        fs::create_dir_all(work.join("skills/demo")).unwrap();
        fs::write(work.join("README.md"), "root\n").unwrap();
        fs::write(
            work.join("skills/demo/SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\n",
        )
        .unwrap();
        git.add_all(&work).unwrap();
        git.commit(&work, "Initial skill").unwrap();
        git.set_origin_url(&work, remote.to_str().unwrap()).unwrap();
        git.push_origin_main(&work, true).unwrap();
        remote
    }
}
