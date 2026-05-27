use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitHubSkillSource {
    pub owner: String,
    pub repo: String,
    pub reference: String,
    pub path: String,
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
    let url = Url::parse(input).map_err(|error| error.to_string())?;
    let mut reference = "main".to_string();
    let mut kind = "github".to_string();
    let owner;
    let repo;
    let skill_path;

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
                reference = parts.get(3).copied().unwrap_or("main").to_string();
                skill_path = strip_skill_md(&parts[4..].join("/"));
            } else {
                skill_path = strip_skill_md(&parts[2..].join("/"));
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
            reference = parts[2].to_string();
            skill_path = strip_skill_md(&parts[3..].join("/"));
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
            skill_path = strip_skill_md(&parts[4..].join("/"));
        }
        _ => return Err("Only GitHub URLs are supported".to_string()),
    }

    if owner.is_empty() || repo.is_empty() || skill_path.is_empty() {
        return Err("GitHub URL must point to a skill directory or SKILL.md file".to_string());
    }

    Ok(GitHubSkillSource {
        url: format!("https://github.com/{owner}/{repo}/tree/{reference}/{skill_path}"),
        repo_url: format!("https://github.com/{owner}/{repo}.git"),
        owner,
        repo,
        reference,
        path: skill_path,
        kind,
    })
}

fn path_parts(url: &Url) -> Vec<&str> {
    url.path()
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn strip_skill_md(path: &str) -> String {
    path.strip_suffix("/SKILL.md")
        .or_else(|| path.strip_suffix("/skill.md"))
        .unwrap_or(path)
        .to_string()
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
