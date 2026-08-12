use serde::Serialize;
use std::path::{Component, Path};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitHubSkillSource {
    pub owner: String,
    pub repo: String,
    pub reference: String,
    pub reference_explicit: bool,
    pub path: String,
    pub is_root: bool,
    pub url: String,
    pub repo_url: String,
    pub kind: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GitHubRefKind {
    Branch,
    Tag,
    Commit,
    Unknown,
}

pub fn classify_ref_text(reference: &str) -> GitHubRefKind {
    let value = reference.trim();
    if value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        GitHubRefKind::Commit
    } else {
        GitHubRefKind::Unknown
    }
}

pub fn parse_github_tree_url(input: &str) -> Result<GitHubSkillSource, String> {
    parse_github_skill_url(input)
}

pub fn parse_github_skill_url(input: &str) -> Result<GitHubSkillSource, String> {
    reject_raw_url_path_traversal(input)?;
    let url = Url::parse(input).map_err(|error| error.to_string())?;
    let mut reference = "main".to_string();
    let mut kind = "github".to_string();
    let owner;
    let repo;
    let skill_path;
    let is_root;

    match url.host_str() {
        Some("github.com") => {
            let parts = path_parts(&url);
            if parts.len() < 2 {
                return Err("GitHub URL must include owner and repo".to_string());
            }
            owner = parts[0].to_string();
            repo = trim_git_suffix(parts[1]);

            if parts.get(2) == Some(&"tree") || parts.get(2) == Some(&"blob") {
                kind = parts[2].to_string();
                let (parsed_ref, parsed_path) = split_ref_and_skill_path(&parts[3..]);
                reference = parsed_ref;
                if kind == "blob" && parsed_path.is_empty() {
                    return Err("GitHub blob URL must point to a SKILL.md file".to_string());
                }
                (skill_path, is_root) = normalize_skill_path(&parsed_path);
            } else {
                (skill_path, is_root) = normalize_skill_path(&parts[2..].join("/"));
            }
        }
        Some("raw.githubusercontent.com") => {
            let parts = path_parts(&url);
            if parts.len() < 4 {
                return Err("Raw GitHub URL must include owner, repo, ref, and path".to_string());
            }
            kind = "raw".to_string();
            owner = parts[0].to_string();
            repo = trim_git_suffix(parts[1]);
            let (parsed_ref, parsed_path) = split_ref_and_skill_path(&parts[2..]);
            reference = parsed_ref;
            (skill_path, is_root) = normalize_skill_path(&parsed_path);
        }
        Some("api.github.com") => {
            let parts = path_parts(&url);
            if parts.len() < 5 || parts[0] != "repos" || parts[3] != "contents" {
                return Err("Unsupported GitHub API URL".to_string());
            }
            kind = "api".to_string();
            owner = parts[1].to_string();
            repo = trim_git_suffix(parts[2]);
            reference = url
                .query_pairs()
                .find(|(key, _)| key == "ref")
                .map(|(_, value)| value.to_string())
                .unwrap_or(reference);
            (skill_path, is_root) = normalize_skill_path(&parts[4..].join("/"));
        }
        _ => return Err("Only GitHub URLs are supported".to_string()),
    }

    if owner.is_empty() || repo.is_empty() || (!is_root && skill_path.is_empty()) {
        return Err("GitHub URL must point to a skill directory or SKILL.md file".to_string());
    }
    if !is_root {
        validate_repo_relative_path(&skill_path)?;
    }
    validate_git_reference(&reference)?;

    let normalized_url = if is_root {
        format!("https://github.com/{owner}/{repo}/tree/{reference}")
    } else {
        format!("https://github.com/{owner}/{repo}/tree/{reference}/{skill_path}")
    };

    Ok(GitHubSkillSource {
        url: normalized_url,
        repo_url: format!("https://github.com/{owner}/{repo}.git"),
        owner,
        repo,
        reference,
        reference_explicit: matches!(kind.as_str(), "tree" | "blob" | "raw")
            || (kind == "api" && url.query_pairs().any(|(key, _)| key == "ref")),
        path: skill_path,
        is_root,
        kind,
    })
}

pub fn validate_repo_relative_path(path: &str) -> Result<(), String> {
    let value = path.trim();
    if value.is_empty() {
        return Err("GitHub path is required.".to_string());
    }
    if value.contains('\\') {
        return Err("GitHub path must use forward slashes.".to_string());
    }
    if Path::new(value).is_absolute() {
        return Err("GitHub path must stay inside the repository.".to_string());
    }
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err("GitHub path must stay inside the repository.".to_string());
        }
    }
    for component in Path::new(value).components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("GitHub path must stay inside the repository.".to_string());
        }
    }
    Ok(())
}

pub fn validate_github_repo_url(repo_url: &str) -> Result<(), String> {
    let value = repo_url.trim();
    if value.is_empty() {
        return Err("GitHub source is missing repoUrl.".to_string());
    }
    if value.starts_with('-') {
        return Err("Git remote URL must not start with '-'.".to_string());
    }
    let url = Url::parse(value)
        .map_err(|_| "Only https://github.com remote URLs are supported.".to_string())?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") {
        return Err("Only https://github.com remote URLs are supported.".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("GitHub remote URL must not contain credentials.".to_string());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("GitHub remote URL must not contain query or fragment.".to_string());
    }
    let parts = path_parts(&url);
    if parts.len() != 2 {
        return Err("GitHub remote URL must include owner and repo.".to_string());
    }
    for part in &parts {
        if part.is_empty() || *part == "." || *part == ".." || part.starts_with('-') {
            return Err("GitHub remote URL must include a valid owner and repo.".to_string());
        }
    }
    Ok(())
}

/// Return a stable, credential-free identity for a GitHub repository URL.
///
/// This intentionally accepts only the HTTPS GitHub repository form used by
/// the local installer lockfile. It does not resolve the repository or imply
/// that the repository can be fetched.
pub fn normalize_github_repo_url(repo_url: &str) -> Result<String, String> {
    validate_github_repo_url(repo_url)?;
    let url = Url::parse(repo_url.trim()).map_err(|error| error.to_string())?;
    let parts = path_parts(&url);
    let owner = parts
        .first()
        .ok_or_else(|| "GitHub remote URL must include an owner.".to_string())?;
    let repo = trim_git_suffix(
        parts
            .get(1)
            .ok_or_else(|| "GitHub remote URL must include a repository.".to_string())?,
    );
    Ok(format!(
        "https://github.com/{}/{}",
        owner.to_ascii_lowercase(),
        repo.to_ascii_lowercase()
    ))
}

pub fn validate_git_reference(reference: &str) -> Result<(), String> {
    let value = reference.trim();
    if value.is_empty() {
        return Err("Git reference is required.".to_string());
    }
    if value.starts_with('-') {
        return Err("Git reference must not start with '-'.".to_string());
    }
    if value.contains('\0') || value.chars().any(|character| character.is_control()) {
        return Err("Git reference contains invalid characters.".to_string());
    }
    Ok(())
}

fn path_parts(url: &Url) -> Vec<&str> {
    url.path()
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn reject_raw_url_path_traversal(input: &str) -> Result<(), String> {
    let Some(after_scheme) = input.split_once("://").map(|(_, rest)| rest) else {
        return Ok(());
    };
    let raw_path = after_scheme
        .split_once('/')
        .map(|(_, path)| path)
        .unwrap_or("");
    let raw_path = raw_path.split(['?', '#']).next().unwrap_or("");
    for segment in raw_path.split('/') {
        let segment = segment.to_ascii_lowercase();
        let decoded = segment.replace("%2e", ".");
        if decoded == "." || decoded == ".." {
            return Err("GitHub path must stay inside the repository.".to_string());
        }
    }
    Ok(())
}

fn split_ref_and_skill_path(parts: &[&str]) -> (String, String) {
    if parts.is_empty() {
        return ("main".to_string(), String::new());
    }

    if let Some(index) = known_skill_path_start(parts) {
        return (parts[..index].join("/"), parts[index..].join("/"));
    }

    (
        parts[0].to_string(),
        parts.get(1..).unwrap_or_default().join("/"),
    )
}

fn known_skill_path_start(parts: &[&str]) -> Option<usize> {
    (1..parts.len()).find(|&index| {
        parts[index] == "skills"
            || matches!(
                parts.get(index..index + 2),
                Some([".agents", "skills"])
                    | Some([".codex", "skills"])
                    | Some([".claude", "skills"])
            )
    })
}

fn normalize_skill_path(path: &str) -> (String, bool) {
    if path == "SKILL.md" || path == "skill.md" {
        return (String::new(), true);
    }

    if path.is_empty() {
        return (String::new(), true);
    }

    (
        path.strip_suffix("/SKILL.md")
            .or_else(|| path.strip_suffix("/skill.md"))
            .unwrap_or(path)
            .to_string(),
        false,
    )
}

fn trim_git_suffix(repo: &str) -> String {
    repo.strip_suffix(".git").unwrap_or(repo).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tree_url() {
        let source = parse_github_skill_url(
            "https://github.com/openai/skills/tree/main/skills/.curated/example",
        )
        .unwrap();

        assert_eq!(source.owner, "openai");
        assert_eq!(source.repo, "skills");
        assert_eq!(source.reference, "main");
        assert_eq!(source.path, "skills/.curated/example");
        assert!(!source.is_root);
    }

    #[test]
    fn rejects_repo_relative_path_traversal() {
        let error =
            parse_github_skill_url("https://github.com/acme/repo/tree/main/skills/../../secret")
                .unwrap_err();

        assert!(error.contains("path must stay inside the repository"));
    }

    #[test]
    fn parses_tree_url_with_slash_ref_when_skill_root_is_known() {
        let source =
            parse_github_skill_url("https://github.com/acme/repo/tree/release/1.0/skills/demo")
                .unwrap();

        assert_eq!(source.reference, "release/1.0");
        assert_eq!(source.path, "skills/demo");
        assert_eq!(
            source.url,
            "https://github.com/acme/repo/tree/release/1.0/skills/demo"
        );
    }

    #[test]
    fn normalizes_blob_raw_and_api_urls_to_skill_directory() {
        assert_eq!(
            parse_github_skill_url("https://github.com/acme/repo/blob/main/skills/demo/SKILL.md")
                .unwrap()
                .path,
            "skills/demo"
        );
        assert_eq!(
            parse_github_skill_url(
                "https://raw.githubusercontent.com/acme/repo/main/skills/demo/SKILL.md"
            )
            .unwrap()
            .path,
            "skills/demo"
        );
        assert_eq!(
            parse_github_skill_url(
                "https://api.github.com/repos/acme/repo/contents/skills/demo/SKILL.md?ref=dev"
            )
            .unwrap()
            .reference,
            "dev"
        );
    }

    #[test]
    fn normalizes_root_skill_md_file_urls_to_repository_root() {
        for url in [
            "https://github.com/acme/repo/blob/main/SKILL.md",
            "https://github.com/acme/repo/blob/main/skill.md",
            "https://github.com/acme/repo/SKILL.md",
            "https://raw.githubusercontent.com/acme/repo/main/SKILL.md",
            "https://raw.githubusercontent.com/acme/repo/main/skill.md",
            "https://api.github.com/repos/acme/repo/contents/SKILL.md?ref=main",
            "https://api.github.com/repos/acme/repo/contents/skill.md?ref=main",
        ] {
            let source = parse_github_skill_url(url).unwrap();

            assert!(source.is_root, "{url}");
            assert_eq!(source.path, "", "{url}");
            assert_eq!(
                source.url, "https://github.com/acme/repo/tree/main",
                "{url}"
            );
        }

        let humanizer =
            parse_github_skill_url("https://github.com/op7418/Humanizer-zh/blob/main/SKILL.md")
                .unwrap();
        assert_eq!(humanizer.owner, "op7418");
        assert_eq!(humanizer.repo, "Humanizer-zh");
        assert_eq!(humanizer.reference, "main");
        assert_eq!(humanizer.path, "");
        assert!(humanizer.is_root);
    }

    #[test]
    fn normalizes_repository_urls_to_root_skill_sources() {
        for url in [
            "https://github.com/acme/repo",
            "https://github.com/acme/repo/",
            "https://github.com/acme/repo.git",
            "https://github.com/acme/repo/tree/main",
        ] {
            let source = parse_github_skill_url(url).unwrap();

            assert!(source.is_root, "{url}");
            assert_eq!(source.path, "", "{url}");
            assert_eq!(source.repo, "repo", "{url}");
            assert_eq!(
                source.url, "https://github.com/acme/repo/tree/main",
                "{url}"
            );
            assert_eq!(source.reference_explicit, url.contains("/tree/"));
        }
    }

    #[test]
    fn normalizes_github_repository_identity_without_credentials_or_git_suffix() {
        assert_eq!(
            normalize_github_repo_url("https://github.com/acme/repo.git").unwrap(),
            "https://github.com/acme/repo"
        );
        assert!(
            normalize_github_repo_url("https://github.com/acme/repo?access_token=secret").is_err()
        );
        assert!(normalize_github_repo_url("https://gitlab.com/acme/repo").is_err());
    }

    #[test]
    fn classifies_commit_ref_without_network() {
        assert_eq!(
            classify_ref_text("0123456789abcdef0123456789abcdef01234567"),
            GitHubRefKind::Commit
        );
    }

    #[test]
    fn non_commit_ref_stays_unknown_until_resolved() {
        assert_eq!(classify_ref_text("main"), GitHubRefKind::Unknown);
        assert_eq!(classify_ref_text("v1.0.0"), GitHubRefKind::Unknown);
    }
}
