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
}
