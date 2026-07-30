use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const DEFAULT_LS_REMOTE_TIMEOUT: Duration = Duration::from_secs(30);
const FETCH_REF_TIMEOUT: Duration = Duration::from_secs(30);
const PUSH_TIMEOUT: Duration = Duration::from_secs(30);
const LOCAL_GIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REF_DIFF_FILES: usize = 500;
const MAX_DIFF_BYTES_PER_FILE: usize = 256 * 1024;
const MAX_TREE_ENTRIES: usize = 20_000;
const MAX_SHOW_FILE_BYTES: usize = 2 * 1024 * 1024;
const BACKUP_REF_PREFIX: &str = "refs/skillbox/backups/";
static COMMAND_OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(0);

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
pub struct GitAheadBehind {
    pub ahead: u64,
    pub behind: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitTreeEntry {
    pub mode: String,
    pub object_type: String,
    pub object_id: String,
    pub size: Option<u64>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBackupRef {
    pub reference: String,
    pub target: String,
    pub previous_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitMergeTreeAnalysis {
    pub tree_id: String,
    pub conflict_files: Vec<String>,
}

impl GitMergeTreeAnalysis {
    pub fn is_clean(&self) -> bool {
        self.conflict_files.is_empty()
    }
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

    pub fn status_hardened(&self, repo: impl AsRef<Path>) -> Result<GitStatus, String> {
        let repo = repo.as_ref();
        if !repo.join(".git").exists() {
            return Ok(GitStatus {
                initialized: false,
                branch: String::new(),
                dirty: false,
                raw_status: String::new(),
            });
        }
        let branch = self.success_stdout(self.run_hardened_status_output(
            repo,
            &["branch", "--show-current"],
            LOCAL_GIT_TIMEOUT,
            "git branch",
        )?)?;
        let head = self.hardened_head(repo)?;
        let index_dirty = match head.as_deref() {
            Some(head) => {
                let output = self.run_hardened_status_output(
                    repo,
                    &[
                        "diff-index",
                        "--cached",
                        "--quiet",
                        "--no-ext-diff",
                        "--no-textconv",
                        head,
                        "--",
                    ],
                    LOCAL_GIT_TIMEOUT,
                    "git diff-index",
                )?;
                if output.status.success() {
                    false
                } else if output.status.code() == Some(1) {
                    true
                } else {
                    return Err(sanitize_git_error(
                        String::from_utf8_lossy(&output.stderr).trim(),
                    ));
                }
            }
            None => {
                let output = self.run_hardened_status_output(
                    repo,
                    &["ls-files", "-z", "--cached"],
                    LOCAL_GIT_TIMEOUT,
                    "git ls-files index",
                )?;
                !self.success_stdout_bytes(output)?.is_empty()
            }
        };
        let worktree_dirty = match head.as_deref() {
            Some(head) => {
                let mut dirty = false;
                for entry in self.hardened_tree_entries(repo, head)? {
                    if !self.worktree_entry_matches(repo, &entry)? {
                        dirty = true;
                        break;
                    }
                }
                dirty
            }
            None => false,
        };
        let untracked = self.untracked_paths(repo)?;
        let dirty = index_dirty || worktree_dirty || !untracked.is_empty();
        let mut raw_status = format!("## {}", branch.trim());
        if index_dirty {
            raw_status.push_str("\n!! index differs from HEAD");
        }
        if worktree_dirty {
            raw_status.push_str("\n!! worktree differs from HEAD");
        }
        for path in untracked {
            raw_status.push_str("\n?? ");
            raw_status.push_str(&path);
        }
        Ok(GitStatus {
            initialized: true,
            branch: branch.trim().to_string(),
            dirty,
            raw_status,
        })
    }

    fn worktree_entry_matches(&self, repo: &Path, entry: &GitTreeEntry) -> Result<bool, String> {
        let target = repo.join(&entry.path);
        let metadata = match fs::symlink_metadata(&target) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        match entry.mode.as_str() {
            "100644" | "100755" => {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Ok(false);
                }
                let executable = metadata.permissions().mode() & 0o111 != 0;
                let args = ["hash-object", "--no-filters", "--", entry.path.as_str()];
                let output = self.run_hardened_status_output(
                    repo,
                    &args,
                    LOCAL_GIT_TIMEOUT,
                    "git hash-object",
                )?;
                let object_id = self.success_stdout(output)?;
                Ok(object_id.trim() == entry.object_id && executable == (entry.mode == "100755"))
            }
            "120000" => {
                if !metadata.file_type().is_symlink() {
                    return Ok(false);
                }
                let args = ["cat-file", "blob", entry.object_id.as_str()];
                let expected = self.success_stdout_bytes(self.run_hardened_status_output(
                    repo,
                    &args,
                    LOCAL_GIT_TIMEOUT,
                    "git cat-file",
                )?)?;
                Ok(fs::read_link(target)
                    .map_err(|error| error.to_string())?
                    .as_os_str()
                    .as_encoded_bytes()
                    == expected)
            }
            _ => Ok(false),
        }
    }

    fn hardened_head(&self, repo: &Path) -> Result<Option<String>, String> {
        let output = self.run_hardened_status_output(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                "HEAD^{commit}",
            ],
            LOCAL_GIT_TIMEOUT,
            "git rev-parse",
        )?;
        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok((!sha.is_empty()).then_some(sha));
        }
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Err(sanitize_git_error(
            String::from_utf8_lossy(&output.stderr).trim(),
        ))
    }

    fn hardened_tree_entries(
        &self,
        repo: &Path,
        revision: &str,
    ) -> Result<Vec<GitTreeEntry>, String> {
        let args = ["ls-tree", "-r", "-z", "--full-tree", "--long", revision];
        let output =
            self.run_hardened_status_output(repo, &args, LOCAL_GIT_TIMEOUT, "git ls-tree")?;
        parse_tree_entries(&self.success_stdout_bytes(output)?)
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

    pub fn fetch_origin_main(&self, repo: impl AsRef<Path>) -> Result<Option<String>, String> {
        self.fetch_origin_main_with_timeout(repo, FETCH_REF_TIMEOUT)
    }

    pub fn fetch_origin_main_with_timeout(
        &self,
        repo: impl AsRef<Path>,
        timeout: Duration,
    ) -> Result<Option<String>, String> {
        let repo = repo.as_ref();
        let args = [
            "fetch",
            "--no-tags",
            "--force",
            "origin",
            "refs/heads/main:refs/remotes/origin/main",
        ];
        let output = self.run_network_output(repo, &args, timeout, "git fetch origin main")?;
        if output.status.success() {
            return self.rev_parse_optional(repo, "refs/remotes/origin/main");
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("couldn't find remote ref refs/heads/main") {
            self.delete_ref_if_present(repo, "refs/remotes/origin/main")?;
            return Ok(None);
        }

        Err(sanitize_git_error(stderr.trim()))
    }

    pub fn rev_parse_optional(
        &self,
        repo: impl AsRef<Path>,
        revision: &str,
    ) -> Result<Option<String>, String> {
        validate_git_revision_arg(revision)?;
        let revision = format!("{revision}^{{commit}}");
        let args = [
            "rev-parse".to_string(),
            "--verify".to_string(),
            "--quiet".to_string(),
            "--end-of-options".to_string(),
            revision,
        ];
        let output = self.run_owned_output_with_timeout(
            repo.as_ref(),
            &args,
            LOCAL_GIT_TIMEOUT,
            "git rev-parse",
        )?;
        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok((!sha.is_empty()).then_some(sha));
        }
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Err(sanitize_git_error(
            String::from_utf8_lossy(&output.stderr).trim(),
        ))
    }

    pub fn merge_base(
        &self,
        repo: impl AsRef<Path>,
        left: &str,
        right: &str,
    ) -> Result<Option<String>, String> {
        let repo = repo.as_ref();
        let left = self.require_commit(repo, left)?;
        let right = self.require_commit(repo, right)?;
        let args = ["merge-base", "--", left.as_str(), right.as_str()];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git merge-base")?;
        if output.status.success() {
            return Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ));
        }
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        Err(sanitize_git_error(
            String::from_utf8_lossy(&output.stderr).trim(),
        ))
    }

    pub fn ahead_behind(
        &self,
        repo: impl AsRef<Path>,
        local: &str,
        upstream: &str,
    ) -> Result<(u64, u64), String> {
        let counts = self.ahead_behind_summary(repo, local, upstream)?;
        Ok((counts.ahead, counts.behind))
    }

    pub fn ahead_behind_summary(
        &self,
        repo: impl AsRef<Path>,
        local: &str,
        upstream: &str,
    ) -> Result<GitAheadBehind, String> {
        let repo = repo.as_ref();
        let local = self.require_commit(repo, local)?;
        let upstream = self.require_commit(repo, upstream)?;
        let range = format!("{local}...{upstream}");
        let args = ["rev-list", "--left-right", "--count", range.as_str()];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git rev-list")?;
        let counts = self.success_stdout(output)?;
        let mut parts = counts.split_whitespace();
        let ahead = parts
            .next()
            .ok_or("Git did not return an ahead count.")?
            .parse::<u64>()
            .map_err(|_| "Git returned an invalid ahead count.".to_string())?;
        let behind = parts
            .next()
            .ok_or("Git did not return a behind count.")?
            .parse::<u64>()
            .map_err(|_| "Git returned an invalid behind count.".to_string())?;
        Ok(GitAheadBehind { ahead, behind })
    }

    pub fn commit_count(&self, repo: impl AsRef<Path>, revision: &str) -> Result<u32, String> {
        let repo = repo.as_ref();
        let revision = self.require_commit(repo, revision)?;
        let args = ["rev-list", "--count", revision.as_str()];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git rev-list")?;
        self.success_stdout(output)?
            .trim()
            .parse::<u32>()
            .map_err(|_| "Git returned an invalid commit count.".to_string())
    }

    pub fn is_ancestor(
        &self,
        repo: impl AsRef<Path>,
        ancestor: &str,
        descendant: &str,
    ) -> Result<bool, String> {
        let repo = repo.as_ref();
        let ancestor = self.require_commit(repo, ancestor)?;
        let descendant = self.require_commit(repo, descendant)?;
        let args = [
            "merge-base",
            "--is-ancestor",
            ancestor.as_str(),
            descendant.as_str(),
        ];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git merge-base")?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(sanitize_git_error(
                String::from_utf8_lossy(&output.stderr).trim(),
            )),
        }
    }

    pub fn diff_refs(
        &self,
        repo: impl AsRef<Path>,
        old_revision: &str,
        new_revision: &str,
    ) -> Result<Vec<GitDiffFile>, String> {
        let repo = repo.as_ref();
        let old_sha = self.require_commit(repo, old_revision)?;
        let new_sha = self.require_commit(repo, new_revision)?;
        let args = [
            "diff",
            "--name-status",
            "-z",
            "-M",
            "--no-ext-diff",
            old_sha.as_str(),
            new_sha.as_str(),
        ];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git diff refs")?;
        let name_status = self.success_stdout_bytes(output)?;
        let entries = parse_name_status_z(&name_status)?;
        if entries.len() > MAX_REF_DIFF_FILES {
            return Err(format!(
                "Git diff contains more than {MAX_REF_DIFF_FILES} files."
            ));
        }

        entries
            .into_iter()
            .map(|(status, old_path, path)| {
                let mut args = vec![
                    "diff".to_string(),
                    "--no-ext-diff".to_string(),
                    "--no-textconv".to_string(),
                    "--no-color".to_string(),
                    "-M".to_string(),
                    "--unified=3".to_string(),
                    old_sha.clone(),
                    new_sha.clone(),
                    "--".to_string(),
                ];
                if let Some(previous) = old_path.as_ref() {
                    args.push(previous.clone());
                }
                args.push(path.clone());
                let output = self.run_owned_output_with_timeout(
                    repo,
                    &args,
                    LOCAL_GIT_TIMEOUT,
                    "git diff file",
                )?;
                let diff = self.success_stdout_bytes(output)?;
                Ok(GitDiffFile {
                    path,
                    old_path,
                    status,
                    diff: bounded_lossy_text(&diff, MAX_DIFF_BYTES_PER_FILE),
                })
            })
            .collect()
    }

    pub fn create_or_update_backup_ref(
        &self,
        repo: impl AsRef<Path>,
        backup_ref: &str,
        target: &str,
    ) -> Result<GitBackupRef, String> {
        validate_backup_ref(backup_ref)?;
        let repo = repo.as_ref();
        let target = self.require_commit(repo, target)?;
        let previous_target = self.rev_parse_optional(repo, backup_ref)?;
        let mut args = vec![
            "update-ref".to_string(),
            "--create-reflog".to_string(),
            backup_ref.to_string(),
            target.clone(),
        ];
        if let Some(previous) = previous_target.as_ref() {
            args.push(previous.clone());
        }
        let output =
            self.run_owned_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git update-ref")?;
        self.success_stdout(output)?;
        Ok(GitBackupRef {
            reference: backup_ref.to_string(),
            target,
            previous_target,
        })
    }

    pub fn create_backup_ref(
        &self,
        repo: impl AsRef<Path>,
        backup_ref: &str,
        target: &str,
    ) -> Result<(), String> {
        self.create_or_update_backup_ref(repo, backup_ref, target)?;
        Ok(())
    }

    pub fn update_main_ref_cas(
        &self,
        repo: impl AsRef<Path>,
        new_revision: &str,
        expected_revision: Option<&str>,
    ) -> Result<String, String> {
        let repo = repo.as_ref();
        self.require_main_symbolic_head(repo)?;
        let new_revision = self.require_commit(repo, new_revision)?;
        let expected = match expected_revision {
            Some(revision) => self.require_commit(repo, revision)?,
            None => "0000000000000000000000000000000000000000".to_string(),
        };
        let args = [
            "update-ref",
            "refs/heads/main",
            new_revision.as_str(),
            expected.as_str(),
        ];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git update-ref main")?;
        self.success_stdout(output)?;
        Ok(new_revision)
    }

    pub fn delete_main_ref_cas(
        &self,
        repo: impl AsRef<Path>,
        expected_revision: &str,
    ) -> Result<(), String> {
        let repo = repo.as_ref();
        self.require_main_symbolic_head(repo)?;
        let expected = self.require_commit(repo, expected_revision)?;
        let args = ["update-ref", "-d", "refs/heads/main", expected.as_str()];
        let output = self.run_output_with_timeout(
            repo,
            &args,
            LOCAL_GIT_TIMEOUT,
            "git update-ref delete main",
        )?;
        self.success_stdout(output)?;
        Ok(())
    }

    pub fn read_tree(&self, repo: impl AsRef<Path>, revision: &str) -> Result<(), String> {
        let repo = repo.as_ref();
        let revision = self.require_commit(repo, revision)?;
        let args = ["read-tree", revision.as_str()];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git read-tree")?;
        self.success_stdout(output)?;
        Ok(())
    }

    pub fn read_empty_tree(&self, repo: impl AsRef<Path>) -> Result<(), String> {
        let output = self.run_output_with_timeout(
            repo.as_ref(),
            &["read-tree", "--empty"],
            LOCAL_GIT_TIMEOUT,
            "git read-tree empty",
        )?;
        self.success_stdout(output)?;
        Ok(())
    }

    pub fn prepare_index_tree(
        &self,
        repo: impl AsRef<Path>,
        revision: Option<&str>,
        index_path: &Path,
    ) -> Result<(), String> {
        let repo = repo.as_ref();
        match fs::remove_file(index_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        let index_path = index_path
            .to_str()
            .ok_or_else(|| "Git index path is not valid UTF-8.".to_string())?;
        let revision = match revision {
            Some(revision) => Some(self.require_commit(repo, revision)?),
            None => None,
        };
        let args = revision
            .as_deref()
            .map(|revision| vec!["read-tree", revision])
            .unwrap_or_else(|| vec!["read-tree", "--empty"]);
        let output = self.run_output_with_timeout_env(
            repo,
            &args,
            LOCAL_GIT_TIMEOUT,
            "git prepare index tree",
            &[("GIT_INDEX_FILE", index_path)],
        )?;
        self.success_stdout(output)?;
        Ok(())
    }

    pub fn untracked_and_ignored_paths(
        &self,
        repo: impl AsRef<Path>,
    ) -> Result<Vec<String>, String> {
        let repo = repo.as_ref();
        let mut paths = Vec::new();
        for args in [
            &["ls-files", "-z", "--others", "--exclude-standard"][..],
            &[
                "ls-files",
                "-z",
                "--others",
                "--ignored",
                "--exclude-standard",
            ][..],
        ] {
            let output = self.run_output_with_timeout(
                repo,
                args,
                LOCAL_GIT_TIMEOUT,
                "git ls-files local content",
            )?;
            let bytes = self.success_stdout_bytes(output)?;
            for path in bytes
                .split(|byte| *byte == 0)
                .filter(|path| !path.is_empty())
            {
                let path = std::str::from_utf8(path)
                    .map_err(|_| "Git returned a non-UTF-8 local path.".to_string())?;
                validate_git_tree_path(path)?;
                paths.push(path.to_string());
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    fn untracked_paths(&self, repo: &Path) -> Result<Vec<String>, String> {
        let output = self.run_hardened_status_output(
            repo,
            &["ls-files", "-z", "--others", "--exclude-standard"],
            LOCAL_GIT_TIMEOUT,
            "git ls-files untracked",
        )?;
        let bytes = self.success_stdout_bytes(output)?;
        let mut paths = Vec::new();
        for path in bytes
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = std::str::from_utf8(path)
                .map_err(|_| "Git returned a non-UTF-8 untracked path.".to_string())?;
            validate_git_tree_path(path)?;
            paths.push(path.to_string());
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub fn fast_forward_only_merge(
        &self,
        repo: impl AsRef<Path>,
        target: &str,
    ) -> Result<String, String> {
        let repo = repo.as_ref();
        let target = self.require_commit(repo, target)?;
        let args = [
            "merge",
            "--ff-only",
            "--no-edit",
            "--no-verify",
            target.as_str(),
        ];
        let output = self.run_output_with_timeout_env(
            repo,
            &args,
            LOCAL_GIT_TIMEOUT,
            "git merge --ff-only",
            &[("GIT_EDITOR", "true"), ("GIT_MERGE_AUTOEDIT", "no")],
        )?;
        self.success_stdout(output)?;
        self.require_commit(repo, "HEAD")
    }

    pub fn fast_forward_only(&self, repo: impl AsRef<Path>, target: &str) -> Result<(), String> {
        self.fast_forward_only_merge(repo, target)?;
        Ok(())
    }

    pub fn initialize_unborn_main_from_ref(
        &self,
        repo: impl AsRef<Path>,
        target: &str,
    ) -> Result<String, String> {
        let repo = repo.as_ref();
        let target = self.require_commit(repo, target)?;
        if self.rev_parse_optional(repo, "HEAD")?.is_some() {
            return Err("Git repository already has a HEAD commit.".to_string());
        }
        self.require_main_symbolic_head(repo)?;
        let status = self.worktree_status_porcelain(repo)?;
        if !status.is_empty() && status != b"?? .gitignore\0" {
            return Err(
                "Unborn Git repository must be empty except for an untracked .gitignore."
                    .to_string(),
            );
        }

        let args = [
            "checkout",
            "--no-guess",
            "--no-track",
            "-b",
            "main",
            target.as_str(),
        ];
        let output = self.run_output_with_timeout(
            repo,
            &args,
            LOCAL_GIT_TIMEOUT,
            "git checkout unborn main",
        )?;
        self.success_stdout(output)?;
        self.require_commit(repo, "HEAD")
    }

    pub fn bootstrap_from_ref(
        &self,
        repo: impl AsRef<Path>,
        branch: &str,
        target: &str,
    ) -> Result<(), String> {
        validate_main_branch(branch)?;
        self.initialize_unborn_main_from_ref(repo, target)?;
        Ok(())
    }

    pub fn restore_worktree_to_ref(
        &self,
        repo: impl AsRef<Path>,
        branch: &str,
        old_revision: &str,
    ) -> Result<String, String> {
        validate_main_branch(branch)?;
        let repo = repo.as_ref();
        self.require_main_symbolic_head(repo)?;
        let current = self.require_commit(repo, "HEAD")?;
        let old = self.require_commit(repo, old_revision)?;
        if !self.worktree_status_porcelain(repo)?.is_empty() {
            return Err("Cannot restore a dirty Git worktree.".to_string());
        }
        if !self.is_ancestor(repo, &old, &current)? {
            return Err(
                "Compensation restore target must be an ancestor of the current HEAD.".to_string(),
            );
        }
        if old == current {
            return Ok(current);
        }

        let args = ["checkout", "--no-guess", "-B", "main", old.as_str()];
        let output = self.run_output_with_timeout(
            repo,
            &args,
            LOCAL_GIT_TIMEOUT,
            "git checkout compensation ref",
        )?;
        self.success_stdout(output)?;
        self.require_commit(repo, "HEAD")
    }

    pub fn restore_unborn_main(
        &self,
        repo: impl AsRef<Path>,
        expected_current: &str,
    ) -> Result<(), String> {
        let repo = repo.as_ref();
        self.require_main_symbolic_head(repo)?;
        let current = self.require_commit(repo, "HEAD")?;
        let expected = self.require_commit(repo, expected_current)?;
        if current != expected {
            return Err("Git HEAD changed before unborn compensation.".to_string());
        }
        if !self.worktree_status_porcelain(repo)?.is_empty() {
            return Err("Cannot restore an unborn state from a dirty Git worktree.".to_string());
        }
        let entries = self.list_tree(repo, &current)?;
        for entry in &entries {
            if entry.object_type != "blob" || !matches!(entry.mode.as_str(), "100644" | "100755") {
                return Err(
                    "Cannot restore unborn state with unsupported tracked file types.".to_string(),
                );
            }
            validate_git_tree_path(&entry.path)?;
            let metadata = fs::symlink_metadata(repo.join(&entry.path)).map_err(|error| {
                format!("Unable to inspect tracked path during restore: {error}")
            })?;
            if !metadata.is_file() {
                return Err(
                    "Cannot restore unborn state because a tracked path changed type.".to_string(),
                );
            }
        }
        let output = self.run_output_with_timeout(
            repo,
            &["update-ref", "-d", "refs/heads/main", current.as_str()],
            LOCAL_GIT_TIMEOUT,
            "git update-ref",
        )?;
        self.success_stdout(output)?;
        for entry in entries {
            let path = repo.join(&entry.path);
            fs::remove_file(&path)
                .map_err(|error| format!("Unable to remove compensated tracked file: {error}"))?;
            remove_empty_parents(&path, repo)?;
        }
        Ok(())
    }

    pub fn merge_tree_analysis(
        &self,
        repo: impl AsRef<Path>,
        left: &str,
        right: &str,
    ) -> Result<GitMergeTreeAnalysis, String> {
        let repo = repo.as_ref();
        let left = self.require_commit(repo, left)?;
        let right = self.require_commit(repo, right)?;
        let args = [
            "merge-tree",
            "--write-tree",
            "--name-only",
            "-z",
            "--no-messages",
            left.as_str(),
            right.as_str(),
        ];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git merge-tree")?;
        if !matches!(output.status.code(), Some(0 | 1)) {
            return Err(sanitize_git_error(
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
        parse_merge_tree_analysis(&output.stdout)
    }

    pub fn list_tree(
        &self,
        repo: impl AsRef<Path>,
        revision: &str,
    ) -> Result<Vec<GitTreeEntry>, String> {
        let repo = repo.as_ref();
        let sha = self.require_commit(repo, revision)?;
        let args = ["ls-tree", "-r", "-z", "--full-tree", "--long", sha.as_str()];
        let output = self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git ls-tree")?;
        let stdout = self.success_stdout_bytes(output)?;
        parse_tree_entries(&stdout)
    }

    pub fn show_file(
        &self,
        repo: impl AsRef<Path>,
        revision: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        validate_git_tree_path(path)?;
        let repo = repo.as_ref();
        let sha = self.require_commit(repo, revision)?;
        let object = format!("{sha}:{path}");
        let check_args = ["cat-file", "-e", object.as_str()];
        let check =
            self.run_output_with_timeout(repo, &check_args, LOCAL_GIT_TIMEOUT, "git cat-file")?;
        if check.status.code() == Some(1) || check.status.code() == Some(128) {
            return Ok(None);
        }
        self.success_stdout(check)?;

        let size_args = ["cat-file", "-s", object.as_str()];
        let size_output =
            self.run_output_with_timeout(repo, &size_args, LOCAL_GIT_TIMEOUT, "git cat-file")?;
        let size = self
            .success_stdout(size_output)?
            .trim()
            .parse::<u64>()
            .map_err(|_| "Git returned an invalid blob size.".to_string())?;
        if size > MAX_SHOW_FILE_BYTES as u64 {
            return Err(format!(
                "Git file exceeds the {MAX_SHOW_FILE_BYTES} byte preview limit."
            ));
        }

        let args = ["cat-file", "blob", object.as_str()];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git cat-file")?;
        let content = self.success_stdout_bytes(output)?;
        if content.len() as u64 != size {
            return Err(format!(
                "Git blob size changed while reading preview: expected {size} bytes, got {}.",
                content.len()
            ));
        }
        Ok(Some(content))
    }

    pub fn tree_path_exists(
        &self,
        repo: impl AsRef<Path>,
        revision: &str,
        path: &str,
    ) -> Result<bool, String> {
        validate_git_tree_path(path)?;
        let repo = repo.as_ref();
        let sha = self.require_commit(repo, revision)?;
        let object = format!("{sha}:{path}");
        let args = ["cat-file", "-e", object.as_str()];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git cat-file")?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1 | 128) => Ok(false),
            _ => Err(sanitize_git_error(
                String::from_utf8_lossy(&output.stderr).trim(),
            )),
        }
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
            return Err(sanitize_git_error(
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
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

    pub fn fetch_ref_tree(
        &self,
        repo_url: &str,
        reference: &str,
        checkout_root: impl AsRef<Path>,
    ) -> Result<String, String> {
        self.fetch_ref_tree_with_timeout(repo_url, reference, checkout_root, FETCH_REF_TIMEOUT)
    }

    pub fn fetch_ref_tree_with_timeout(
        &self,
        repo_url: &str,
        reference: &str,
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
        self.run(checkout_root, &["checkout", "FETCH_HEAD"])?;
        let git_metadata = checkout_root.join(".git");
        match fs::symlink_metadata(&git_metadata) {
            Ok(metadata) if metadata.is_dir() => {
                fs::remove_dir_all(&git_metadata).map_err(|error| error.to_string())?;
            }
            Ok(_) => {
                fs::remove_file(&git_metadata).map_err(|error| error.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
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
        let output = self.run_network_output(repo, args, timeout, label)?;

        if !output.status.success() {
            return Err(sanitize_git_error(
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn run_network_output(
        &self,
        repo: &Path,
        args: &[&str],
        timeout: Duration,
        label: &str,
    ) -> Result<Output, String> {
        let trusted_credential_helpers = trusted_global_credential_helpers();
        let mut command = Command::new("git");
        command
            .arg("-c")
            .arg("core.hooksPath=/dev/null")
            .arg("-c")
            .arg("credential.helper=")
            .arg("-c")
            .arg("credential.interactive=false")
            .arg("-c")
            .arg("core.sshCommand=ssh")
            .arg("-c")
            .arg("core.gitProxy=")
            .arg("-c")
            .arg("remote.origin.uploadpack=git-upload-pack")
            .arg("-c")
            .arg("protocol.ext.allow=never");
        for helper in trusted_credential_helpers {
            command.arg("-c").arg(format!("credential.helper={helper}"));
        }
        command
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "true")
            .env("GCM_INTERACTIVE", "never")
            .env_remove("GIT_SSH")
            .env_remove("GIT_SSH_COMMAND")
            .env("LC_ALL", "C");
        self.command_output_with_timeout(command, timeout, label)
    }

    fn run_output_with_timeout(
        &self,
        repo: &Path,
        args: &[&str],
        timeout: Duration,
        label: &str,
    ) -> Result<Output, String> {
        self.run_output_with_timeout_env(repo, args, timeout, label, &[])
    }

    fn run_hardened_status_output(
        &self,
        repo: &Path,
        args: &[&str],
        timeout: Duration,
        label: &str,
    ) -> Result<Output, String> {
        let mut command = Command::new("git");
        command
            .arg("--no-optional-locks")
            .arg("-c")
            .arg("core.hooksPath=/dev/null")
            .arg("-c")
            .arg("core.fsmonitor=false")
            .arg("-c")
            .arg("diff.external=")
            .arg("-c")
            .arg("interactive.diffFilter=")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C");
        self.command_output_with_timeout(command, timeout, label)
    }

    fn run_output_with_timeout_env(
        &self,
        repo: &Path,
        args: &[&str],
        timeout: Duration,
        label: &str,
        env: &[(&str, &str)],
    ) -> Result<Output, String> {
        let mut command = Command::new("git");
        command
            .arg("-c")
            .arg("core.hooksPath=/dev/null")
            .arg("-c")
            .arg("diff.external=")
            .arg("-c")
            .arg("interactive.diffFilter=")
            .arg("-c")
            .arg("core.fsmonitor=false")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("LC_ALL", "C");
        command.envs(env.iter().copied());
        self.command_output_with_timeout(command, timeout, label)
    }

    fn run_owned_output_with_timeout(
        &self,
        repo: &Path,
        args: &[String],
        timeout: Duration,
        label: &str,
    ) -> Result<Output, String> {
        let mut command = Command::new("git");
        command
            .arg("-c")
            .arg("core.hooksPath=/dev/null")
            .arg("-c")
            .arg("diff.external=")
            .arg("-c")
            .arg("interactive.diffFilter=")
            .arg("-c")
            .arg("core.fsmonitor=false")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("LC_ALL", "C");
        self.command_output_with_timeout(command, timeout, label)
    }

    fn success_stdout(&self, output: Output) -> Result<String, String> {
        Ok(String::from_utf8_lossy(&self.success_stdout_bytes(output)?).to_string())
    }

    fn success_stdout_bytes(&self, output: Output) -> Result<Vec<u8>, String> {
        if !output.status.success() {
            return Err(sanitize_git_error(
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
        Ok(output.stdout)
    }

    fn require_commit(&self, repo: &Path, revision: &str) -> Result<String, String> {
        self.rev_parse_optional(repo, revision)?
            .ok_or_else(|| format!("Git revision does not exist: {revision}"))
    }

    fn delete_ref_if_present(&self, repo: &Path, reference: &str) -> Result<(), String> {
        if self.rev_parse_optional(repo, reference)?.is_none() {
            return Ok(());
        }
        let args = ["update-ref", "-d", reference];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git update-ref")?;
        self.success_stdout(output)?;
        Ok(())
    }

    fn require_main_symbolic_head(&self, repo: &Path) -> Result<(), String> {
        let args = ["symbolic-ref", "--quiet", "--short", "HEAD"];
        let output =
            self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git symbolic-ref")?;
        let branch = self.success_stdout(output)?;
        if branch.trim() != "main" {
            return Err("Git operation requires the symbolic main branch.".to_string());
        }
        Ok(())
    }

    fn worktree_status_porcelain(&self, repo: &Path) -> Result<Vec<u8>, String> {
        let args = ["status", "--porcelain=v1", "-z", "--untracked-files=all"];
        let output = self.run_output_with_timeout(repo, &args, LOCAL_GIT_TIMEOUT, "git status")?;
        self.success_stdout_bytes(output)
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
        let output_id = COMMAND_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let output_root = std::env::temp_dir().join(format!(
            "skillbox-git-output-{}-{output_id}",
            std::process::id()
        ));
        fs::create_dir_all(&output_root).map_err(|error| error.to_string())?;
        let stdout_path = output_root.join("stdout");
        let stderr_path = output_root.join("stderr");
        let mut stdout_file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&stdout_path)
            .map_err(|error| error.to_string())?;
        let mut stderr_file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&stderr_path)
            .map_err(|error| error.to_string())?;
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                stdout_file.try_clone().map_err(|error| error.to_string())?,
            ))
            .stderr(Stdio::from(
                stderr_file.try_clone().map_err(|error| error.to_string())?,
            ))
            .process_group(0)
            .spawn()
            .map_err(|error| error.to_string())?;
        let started_at = Instant::now();

        loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                let stdout = read_command_output_file(&mut stdout_file)?;
                let stderr = read_command_output_file(&mut stderr_file)?;
                let _ = fs::remove_dir_all(&output_root);
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }

            if started_at.elapsed() >= timeout {
                terminate_process_group(child.id());
                let cleanup_started = Instant::now();
                while child
                    .try_wait()
                    .map_err(|error| error.to_string())?
                    .is_none()
                    && cleanup_started.elapsed() < Duration::from_millis(500)
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                kill_process_group(child.id());
                let _ = child.wait();
                let _ = fs::remove_dir_all(&output_root);
                return Err(format!(
                    "{label} timed out after {}",
                    format_duration(timeout)
                ));
            }

            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn trusted_global_credential_helpers() -> Vec<String> {
    let output = Command::new("git")
        .args(["config", "--global", "--get-all", "credential.helper"])
        .env("LC_ALL", "C")
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn read_command_output_file(file: &mut fs::File) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn terminate_process_group(pid: u32) {
    // SAFETY: negative pid targets only the isolated process group created for this command.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
}

fn kill_process_group(pid: u32) {
    // SAFETY: negative pid targets only the isolated process group created for this command.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
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
    if millis.is_multiple_of(1000) {
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

pub fn fetch_ref_tree(
    repo_url: &str,
    reference: &str,
    checkout_root: impl AsRef<Path>,
) -> Result<String, String> {
    GitService::new().fetch_ref_tree(repo_url, reference, checkout_root)
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

fn validate_git_revision_arg(revision: &str) -> Result<(), String> {
    if revision.is_empty() {
        return Err("Git revision is required.".to_string());
    }
    if revision.len() > 1024 {
        return Err("Git revision is too long.".to_string());
    }
    if revision.starts_with('-')
        || revision.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || !matches!(
                    character,
                    'a'..='z'
                        | 'A'..='Z'
                        | '0'..='9'
                        | '/'
                        | '.'
                        | '-'
                        | '_'
                )
        })
        || revision.contains("..")
        || revision.ends_with('.')
        || revision.ends_with('/')
        || revision.contains("//")
    {
        return Err("Git revision contains unsupported syntax.".to_string());
    }
    Ok(())
}

fn validate_backup_ref(reference: &str) -> Result<(), String> {
    validate_git_revision_arg(reference)?;
    if !reference.starts_with(BACKUP_REF_PREFIX)
        || reference == BACKUP_REF_PREFIX
        || reference.ends_with(".lock")
        || reference
            .split('/')
            .any(|component| component.is_empty() || component.starts_with('.'))
    {
        return Err(format!(
            "Backup ref must be under {BACKUP_REF_PREFIX} and use a valid ref name."
        ));
    }
    Ok(())
}

fn validate_main_branch(branch: &str) -> Result<(), String> {
    if branch != "main" && branch != "refs/heads/main" {
        return Err("Git apply primitive only supports the main branch.".to_string());
    }
    Ok(())
}

fn validate_git_tree_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 4096 {
        return Err("Git tree path is invalid.".to_string());
    }
    if path.contains(':')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || Path::new(path).is_absolute()
    {
        return Err("Git tree path is invalid.".to_string());
    }
    for component in Path::new(path).components() {
        match component {
            std::path::Component::Normal(value) if value != ".git" => {}
            _ => return Err("Git tree path must be a safe repository-relative path.".to_string()),
        }
    }
    Ok(())
}

fn remove_empty_parents(path: &Path, boundary: &Path) -> Result<(), String> {
    let mut current = path.parent();
    while let Some(directory) = current {
        if directory == boundary || directory == boundary.join(".git") {
            break;
        }
        match fs::remove_dir(directory) {
            Ok(()) => current = directory.parent(),
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                current = directory.parent()
            }
            Err(error) => return Err(format!("Unable to prune compensated directory: {error}")),
        }
    }
    Ok(())
}

fn parse_name_status_z(output: &[u8]) -> Result<Vec<(String, Option<String>, String)>, String> {
    let fields = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| {
            std::str::from_utf8(field)
                .map(str::to_string)
                .map_err(|_| "Git diff returned a non-UTF-8 path.".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut fields = fields.into_iter();
    let mut entries = Vec::new();
    while let Some(status_code) = fields.next() {
        let status = status_code
            .chars()
            .next()
            .ok_or("Git diff returned an empty status.")?
            .to_string();
        if matches!(status.as_str(), "R" | "C") {
            let old_path = fields
                .next()
                .ok_or("Git diff omitted the old rename path.")?;
            let path = fields
                .next()
                .ok_or("Git diff omitted the new rename path.")?;
            entries.push((status, Some(old_path), path));
        } else {
            let path = fields.next().ok_or("Git diff omitted a file path.")?;
            entries.push((status, None, path));
        }
    }
    Ok(entries)
}

fn parse_merge_tree_analysis(output: &[u8]) -> Result<GitMergeTreeAnalysis, String> {
    let mut fields = output.split(|byte| *byte == 0);
    let tree_id = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or("Git merge-tree omitted the result tree.")?;
    let tree_id = std::str::from_utf8(tree_id)
        .map_err(|_| "Git merge-tree returned a non-UTF-8 tree id.")?
        .trim()
        .to_string();
    let conflict_files = fields
        .filter(|field| !field.is_empty())
        .map(|field| {
            std::str::from_utf8(field)
                .map(str::to_string)
                .map_err(|_| "Git merge-tree returned a non-UTF-8 path.".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GitMergeTreeAnalysis {
        tree_id,
        conflict_files,
    })
}

fn parse_tree_entries(output: &[u8]) -> Result<Vec<GitTreeEntry>, String> {
    let records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    if records.len() > MAX_TREE_ENTRIES {
        return Err(format!(
            "Git tree contains more than {MAX_TREE_ENTRIES} entries."
        ));
    }

    records
        .into_iter()
        .map(|record| {
            let record = std::str::from_utf8(record)
                .map_err(|_| "Git tree returned a non-UTF-8 path.".to_string())?;
            let (metadata, path) = record
                .split_once('\t')
                .ok_or("Git tree returned an invalid entry.")?;
            let mut metadata = metadata.split_whitespace();
            let mode = metadata
                .next()
                .ok_or("Git tree omitted an entry mode.")?
                .to_string();
            let object_type = metadata
                .next()
                .ok_or("Git tree omitted an object type.")?
                .to_string();
            let object_id = metadata
                .next()
                .ok_or("Git tree omitted an object id.")?
                .to_string();
            let size = match metadata.next().ok_or("Git tree omitted an entry size.")? {
                "-" => None,
                value => Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| "Git tree returned an invalid entry size.".to_string())?,
                ),
            };
            Ok(GitTreeEntry {
                mode,
                object_type,
                object_id,
                size,
                path: path.to_string(),
            })
        })
        .collect()
}

fn bounded_lossy_text(bytes: &[u8], limit: usize) -> String {
    if bytes.len() <= limit {
        return String::from_utf8_lossy(bytes).to_string();
    }
    let mut text = String::from_utf8_lossy(&bytes[..limit]).to_string();
    text.push_str("\n[diff truncated by SkillBox]\n");
    text
}

fn sanitize_git_error(message: &str) -> String {
    let mut sanitized = message.to_string();
    while let Some(scheme_end) = sanitized.find("://") {
        let start = sanitized[..scheme_end]
            .rfind(|character: char| character.is_whitespace() || matches!(character, '\'' | '"'))
            .map_or(0, |index| index + 1);
        let end = sanitized[scheme_end + 3..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '\'' | '"' | ')' | ']')
            })
            .map_or(sanitized.len(), |index| scheme_end + 3 + index);
        sanitized.replace_range(start..end, "<redacted-remote>");
    }
    sanitized
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
    use std::io::Write as _;
    use std::net::TcpListener;
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
    fn hardened_status_bypasses_repository_commands_and_detects_dirty_state() {
        let git = GitService::new();
        let repo = temp_dir("skillbox-git-hardened-status");
        git.init_main(&repo).unwrap();
        write_file(&repo.join("tracked.txt"), "original\n");
        write_file(
            &repo.join(".gitattributes"),
            "tracked.txt filter=marker diff=marker\n",
        );
        git.add_all(&repo).unwrap();
        git.commit(&repo, "Initial files").unwrap();

        let git_dir = repo.join(".git");
        let command_dir = git_dir.join("status-marker-commands");
        fs::create_dir_all(&command_dir).unwrap();
        let marker_command = |name: &str| {
            let marker = git_dir.join(format!("{name}.invoked"));
            let script = command_dir.join(name);
            fs::write(
                &script,
                format!("#!/bin/sh\nprintf invoked > '{}'\ncat\n", marker.display()),
            )
            .unwrap();
            let mut permissions = fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions).unwrap();
            (script, marker)
        };
        let (clean_command, clean_marker) = marker_command("clean");
        let (smudge_command, smudge_marker) = marker_command("smudge");
        let (external_diff_command, external_diff_marker) = marker_command("external-diff");
        let (textconv_command, textconv_marker) = marker_command("textconv");
        let (fsmonitor_command, fsmonitor_marker) = marker_command("fsmonitor");
        let hooks_dir = git_dir.join("status-marker-hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_marker = git_dir.join("hook.invoked");
        let hook = hooks_dir.join("post-index-change");
        fs::write(
            &hook,
            format!("#!/bin/sh\nprintf invoked > '{}'\n", hook_marker.display()),
        )
        .unwrap();
        let mut hook_permissions = fs::metadata(&hook).unwrap().permissions();
        hook_permissions.set_mode(0o755);
        fs::set_permissions(&hook, hook_permissions).unwrap();

        for (key, value) in [
            ("filter.marker.clean", clean_command.as_path()),
            ("filter.marker.smudge", smudge_command.as_path()),
            ("diff.external", external_diff_command.as_path()),
            ("diff.marker.textconv", textconv_command.as_path()),
            ("core.fsmonitor", fsmonitor_command.as_path()),
            ("core.hooksPath", hooks_dir.as_path()),
        ] {
            run_git(&repo, &["config", key, value.to_str().unwrap()]).unwrap();
        }
        run_git(&repo, &["config", "filter.marker.required", "true"]).unwrap();

        let clean_status = git.status_hardened(&repo).unwrap();
        assert!(!clean_status.dirty);

        write_file(&repo.join("tracked.txt"), "modified\n");
        write_file(&repo.join("untracked.txt"), "new\n");
        let dirty_status = git.status_hardened(&repo).unwrap();

        assert!(dirty_status.dirty);
        assert!(dirty_status
            .raw_status
            .contains("!! worktree differs from HEAD"));
        assert!(dirty_status.raw_status.contains("?? untracked.txt"));
        for marker in [
            clean_marker,
            smudge_marker,
            external_diff_marker,
            textconv_marker,
            fsmonitor_marker,
            hook_marker,
        ] {
            assert!(
                !marker.exists(),
                "hardened status executed repository command marker {}",
                marker.display()
            );
        }
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
    fn snapshot_fetch_ref_tree_checks_out_full_tree() {
        let remote = bare_remote_with_skill("git-snapshot-tree-origin");
        let temp = temp_dir("git-snapshot-tree-work");
        let checkout = temp.join("checkout");

        let sha = fetch_ref_tree(remote.to_str().unwrap(), "main", &checkout).unwrap();

        assert!(!sha.is_empty());
        assert!(checkout.join("skills/demo/SKILL.md").exists());
        assert!(checkout.join("README.md").exists());
        assert!(!checkout.join(".git").exists());
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
    fn command_timeout_terminates_descendant_process_group() {
        let temp = temp_dir("git-timeout-descendant");
        let marker = temp.join("descendant-survived");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("(sleep 1; printf survived > \"$1\") & wait")
            .arg("sh")
            .arg(&marker);

        let started_at = Instant::now();
        let error = GitService::new()
            .command_output_with_timeout(
                command,
                std::time::Duration::from_millis(100),
                "process tree",
            )
            .unwrap_err();

        assert!(error.contains("timed out"));
        assert!(started_at.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(1_100));
        assert!(
            !marker.exists(),
            "the timed-out command must not leave a descendant running"
        );
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

    #[test]
    fn fetch_origin_main_updates_tracking_ref_and_supports_missing_remote_main() {
        let git = GitService::new();
        let remote = bare_remote_with_skill("git-inbound-fetch");
        let repo = temp_dir("git-inbound-fetch-client");
        git.init_main(&repo).unwrap();
        git.set_origin_url(&repo, remote.to_str().unwrap()).unwrap();

        let fetched = git.fetch_origin_main(&repo).unwrap().unwrap();
        assert_eq!(
            git.rev_parse_optional(&repo, "refs/remotes/origin/main")
                .unwrap(),
            Some(fetched)
        );
        assert_eq!(git.rev_parse_optional(&repo, "HEAD").unwrap(), None);

        let empty_remote = temp_dir("git-inbound-empty").join("remote.git");
        Command::new("git")
            .args(["init", "--bare"])
            .arg(&empty_remote)
            .output()
            .unwrap();
        git.set_origin_url(&repo, empty_remote.to_str().unwrap())
            .unwrap();
        assert_eq!(git.fetch_origin_main(&repo).unwrap(), None);
        assert_eq!(
            git.rev_parse_optional(&repo, "refs/remotes/origin/main")
                .unwrap(),
            None
        );
    }

    #[test]
    fn network_fetch_does_not_execute_repository_credential_helper() {
        let git = GitService::new();
        let repo = temp_dir("git-network-credential-helper");
        git.init_main(&repo).unwrap();
        let marker = repo.join("credential-helper-invoked");
        let helper = format!(
            "!sh -c 'printf invoked > \"{}\"; printf \"username=x\\npassword=y\\n\"'",
            marker.display()
        );
        run_git(&repo, &["config", "credential.helper", &helper]).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"test\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        git.set_origin_url(&repo, &format!("http://{address}/repo.git"))
            .unwrap();

        let error = git
            .fetch_origin_main_with_timeout(&repo, Duration::from_secs(5))
            .unwrap_err();
        server.join().unwrap();
        assert!(!error.is_empty());
        assert!(
            !marker.exists(),
            "network fetch executed a repository-local credential helper"
        );
    }

    #[test]
    fn network_fetch_does_not_execute_repository_git_proxy() {
        let git = GitService::new();
        let repo = temp_dir("git-network-proxy");
        git.init_main(&repo).unwrap();
        let marker = repo.join("git-proxy-invoked");
        let proxy = format!("sh -c 'printf invoked > \"{}\"; exit 1'", marker.display());
        run_git(&repo, &["config", "core.gitProxy", &proxy]).unwrap();
        git.set_origin_url(&repo, "git://127.0.0.1:9/repo.git")
            .unwrap();

        let error = git
            .fetch_origin_main_with_timeout(&repo, Duration::from_millis(500))
            .unwrap_err();

        assert!(!error.is_empty());
        assert!(
            !marker.exists(),
            "network fetch executed a repository-local Git proxy"
        );
    }

    #[test]
    fn graph_primitives_report_divergence_and_optional_unborn_refs() {
        let git = GitService::new();
        let repo = temp_dir("git-inbound-graph");
        git.init_main(&repo).unwrap();
        assert_eq!(git.rev_parse_optional(&repo, "HEAD").unwrap(), None);

        write_file(&repo.join("base.txt"), "base\n");
        let base = commit_all(&repo, "base");
        write_file(&repo.join("local.txt"), "local\n");
        let local = commit_all(&repo, "local");
        run_git(&repo, &["checkout", "-b", "remote", &base]).unwrap();
        write_file(&repo.join("remote.txt"), "remote\n");
        let remote = commit_all(&repo, "remote");

        assert_eq!(
            git.merge_base(&repo, &local, &remote).unwrap(),
            Some(base.clone())
        );
        assert_eq!(git.ahead_behind(&repo, &local, &remote).unwrap(), (1, 1));
        assert_eq!(
            git.ahead_behind_summary(&repo, &local, &remote).unwrap(),
            GitAheadBehind {
                ahead: 1,
                behind: 1
            }
        );
        assert_eq!(git.commit_count(&repo, &local).unwrap(), 2);
        assert!(git.is_ancestor(&repo, &base, &local).unwrap());
        assert!(!git.is_ancestor(&repo, &local, &remote).unwrap());
        assert_eq!(
            git.rev_parse_optional(&repo, "refs/heads/missing").unwrap(),
            None
        );
    }

    #[test]
    fn diff_refs_preserves_rename_metadata_and_bounds_file_diff() {
        let git = GitService::new();
        let repo = temp_dir("git-inbound-diff");
        git.init_main(&repo).unwrap();
        write_file(&repo.join("old.txt"), "rename me\n");
        write_file(
            &repo.join("large.txt"),
            &format!("{}\n", "a".repeat(300_000)),
        );
        let old = commit_all(&repo, "old");
        run_git(&repo, &["mv", "old.txt", "new.txt"]).unwrap();
        write_file(
            &repo.join("large.txt"),
            &format!("{}\n", "b".repeat(300_000)),
        );
        let new = commit_all(&repo, "new");

        let files = git.diff_refs(&repo, &old, &new).unwrap();
        let renamed = files.iter().find(|file| file.path == "new.txt").unwrap();
        assert_eq!(renamed.old_path.as_deref(), Some("old.txt"));
        assert_eq!(renamed.status, "R");
        let large = files.iter().find(|file| file.path == "large.txt").unwrap();
        assert!(large.diff.contains("[diff truncated by SkillBox]"));
        assert!(large.diff.len() <= MAX_DIFF_BYTES_PER_FILE + 64);
    }

    #[test]
    fn backup_ref_is_namespaced_and_updates_with_previous_target() {
        let git = GitService::new();
        let repo = temp_dir("git-inbound-backup");
        git.init_main(&repo).unwrap();
        write_file(&repo.join("one"), "one");
        let first = commit_all(&repo, "first");
        write_file(&repo.join("two"), "two");
        let second = commit_all(&repo, "second");

        let created = git
            .create_or_update_backup_ref(&repo, "refs/skillbox/backups/before-sync", &first)
            .unwrap();
        assert_eq!(created.previous_target, None);
        let updated = git
            .create_or_update_backup_ref(&repo, "refs/skillbox/backups/before-sync", &second)
            .unwrap();
        assert_eq!(updated.previous_target, Some(first));
        assert_eq!(updated.target, second);
        assert!(git
            .create_or_update_backup_ref(&repo, "refs/heads/main", "HEAD")
            .unwrap_err()
            .contains(BACKUP_REF_PREFIX));
    }

    #[test]
    fn fast_forward_and_compensation_restore_enforce_ancestor_boundary() {
        let git = GitService::new();
        let repo = temp_dir("git-inbound-apply");
        git.init_main(&repo).unwrap();
        write_file(&repo.join("file"), "old\n");
        let old = commit_all(&repo, "old");
        write_file(&repo.join("file"), "new\n");
        let new = commit_all(&repo, "new");
        run_git(&repo, &["checkout", "-B", "main", &old]).unwrap();

        assert_eq!(git.fast_forward_only_merge(&repo, &new).unwrap(), new);
        assert_eq!(fs::read_to_string(repo.join("file")).unwrap(), "new\n");
        assert_eq!(
            git.restore_worktree_to_ref(&repo, "main", &old).unwrap(),
            old
        );
        assert_eq!(fs::read_to_string(repo.join("file")).unwrap(), "old\n");
        assert!(git
            .restore_worktree_to_ref(&repo, "main", &new)
            .unwrap_err()
            .contains("must be an ancestor"));

        write_file(&repo.join("dirty"), "dirty");
        assert!(git
            .restore_worktree_to_ref(&repo, "main", "HEAD")
            .unwrap_err()
            .contains("dirty"));
    }

    #[test]
    fn unborn_initialization_requires_empty_main_worktree() {
        let git = GitService::new();
        let source = temp_dir("git-inbound-unborn-source");
        git.init_main(&source).unwrap();
        write_file(&source.join("remote.txt"), "remote\n");
        let target = commit_all(&source, "remote");

        let repo = temp_dir("git-inbound-unborn");
        git.init_main(&repo).unwrap();
        run_git(&repo, &["fetch", source.to_str().unwrap(), &target]).unwrap();
        write_file(&repo.join("unexpected.txt"), "local\n");
        assert!(git
            .initialize_unborn_main_from_ref(&repo, &target)
            .unwrap_err()
            .contains("must be empty"));
        fs::remove_file(repo.join("unexpected.txt")).unwrap();
        write_file(&repo.join(".gitignore"), ".DS_Store\n");

        assert_eq!(
            git.initialize_unborn_main_from_ref(&repo, &target).unwrap(),
            target
        );
        assert_eq!(
            fs::read_to_string(repo.join("remote.txt")).unwrap(),
            "remote\n"
        );
        assert!(repo.join(".gitignore").exists());
        assert!(git
            .initialize_unborn_main_from_ref(&repo, "HEAD")
            .unwrap_err()
            .contains("already has a HEAD"));
    }

    #[test]
    fn unborn_compensation_removes_only_reviewed_tracked_tree() {
        let git = GitService::new();
        let source = temp_dir("git-inbound-unborn-restore-source");
        git.init_main(&source).unwrap();
        fs::create_dir_all(source.join("demo")).unwrap();
        write_file(&source.join("demo/SKILL.md"), "demo\n");
        let target = commit_all(&source, "remote");
        let repo = temp_dir("git-inbound-unborn-restore");
        git.init_main(&repo).unwrap();
        run_git(&repo, &["fetch", source.to_str().unwrap(), &target]).unwrap();
        git.initialize_unborn_main_from_ref(&repo, &target).unwrap();

        git.restore_unborn_main(&repo, &target).unwrap();

        assert_eq!(git.rev_parse_optional(&repo, "HEAD").unwrap(), None);
        assert!(!repo.join("demo").exists());
        assert!(repo.join(".git").is_dir());
    }

    #[test]
    fn merge_tree_analysis_reports_conflicts_without_moving_head() {
        let git = GitService::new();
        let repo = temp_dir("git-inbound-merge-tree");
        git.init_main(&repo).unwrap();
        write_file(&repo.join("file"), "base\n");
        let base = commit_all(&repo, "base");
        write_file(&repo.join("file"), "left\n");
        let left = commit_all(&repo, "left");
        run_git(&repo, &["checkout", "-b", "right", &base]).unwrap();
        write_file(&repo.join("file"), "right\n");
        let right = commit_all(&repo, "right");
        let head_before = git.rev_parse_optional(&repo, "HEAD").unwrap();

        let analysis = git.merge_tree_analysis(&repo, &left, &right).unwrap();
        assert!(!analysis.is_clean());
        assert_eq!(analysis.conflict_files, vec!["file"]);
        assert_eq!(git.rev_parse_optional(&repo, "HEAD").unwrap(), head_before);
        assert_eq!(fs::read_to_string(repo.join("file")).unwrap(), "right\n");
    }

    #[test]
    fn list_tree_and_show_file_offer_read_only_bounded_preview() {
        let git = GitService::new();
        let repo = temp_dir("git-inbound-tree-preview");
        git.init_main(&repo).unwrap();
        fs::create_dir_all(repo.join("skills/demo")).unwrap();
        write_file(&repo.join("skills/demo/SKILL.md"), "name: demo\n");
        let sha = commit_all(&repo, "tree");

        let entries = git.list_tree(&repo, &sha).unwrap();
        assert!(entries.iter().any(|entry| {
            entry.path == "skills/demo/SKILL.md"
                && entry.object_type == "blob"
                && entry.mode == "100644"
                && entry.size == Some(11)
        }));
        assert_eq!(
            git.show_file(&repo, &sha, "skills/demo/SKILL.md").unwrap(),
            Some(b"name: demo\n".to_vec())
        );
        assert_eq!(git.show_file(&repo, &sha, "missing").unwrap(), None);
        assert!(git
            .tree_path_exists(&repo, &sha, "skills/demo/SKILL.md")
            .unwrap());
        assert!(!git.tree_path_exists(&repo, &sha, "missing").unwrap());
        for invalid in ["../outside", "/absolute", ".git/config", "bad:selector"] {
            assert!(git.show_file(&repo, &sha, invalid).is_err());
            assert!(git.tree_path_exists(&repo, &sha, invalid).is_err());
        }
        assert_eq!(git.rev_parse_optional(&repo, "HEAD").unwrap(), Some(sha));
    }

    #[test]
    fn git_errors_redact_remote_urls_with_credentials() {
        let error = sanitize_git_error(
            "fatal: unable to access 'https://user:secret@example.com/repo.git/': denied",
        );
        assert!(!error.contains("user"));
        assert!(!error.contains("secret"));
        assert!(!error.contains("example.com"));
        assert!(error.contains("<redacted-remote>"));
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

    fn run_git(repo: &Path, args: &[&str]) -> Result<String, String> {
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

    fn commit_all(repo: &Path, message: &str) -> String {
        run_git(repo, &["add", "."]).unwrap();
        GitService::new().commit(repo, message).unwrap()
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
