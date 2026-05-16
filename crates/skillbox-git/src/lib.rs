use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatus {
    pub initialized: bool,
    pub branch: String,
    pub dirty: bool,
    pub raw_status: String,
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
