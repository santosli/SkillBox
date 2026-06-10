use crate::*;

pub(crate) fn claude_marketplace_api_get() -> Result<String> {
    let args = claude_marketplace_api_curl_args();
    let output = std::process::Command::new("curl")
        .args(&args)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(claude_marketplace_api_error_message(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(crate) fn claude_marketplace_api_curl_args() -> Vec<String> {
    vec![
        "-fsSL".to_string(),
        "-H".to_string(),
        "Accept: application/json".to_string(),
        "-H".to_string(),
        "User-Agent: SkillBox".to_string(),
        CLAUDE_MARKETPLACE_SKILLS_API.to_string(),
    ]
}

pub(crate) fn claude_marketplace_api_error_message(stderr: &str) -> String {
    if stderr.trim().is_empty() {
        "Claude Marketplace source search failed.".to_string()
    } else {
        format!("Claude Marketplace source search failed: {}", stderr.trim())
    }
}

pub(crate) fn fetch_remote_source_skill_path(
    repo_url: &str,
    reference: &str,
    requested_path: &str,
    skill_name: &str,
    checkout: &Path,
) -> Result<(String, String)> {
    let candidates = remote_source_path_candidates(skill_name, requested_path);
    let mut first_error = None;
    let git = skillbox_git::GitService::new();

    for candidate in &candidates {
        if checkout.exists() {
            fs::remove_dir_all(checkout).map_err(|error| error.to_string())?;
        }

        match git.fetch_ref_path(repo_url, reference, candidate, checkout) {
            Ok(sha) => return Ok((sha, candidate.clone())),
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    Err(format!(
        "{}\nTried source paths: {}",
        first_error.unwrap_or_else(|| "Unable to fetch source path.".to_string()),
        candidates.join(", ")
    ))
}

pub(crate) fn remote_source_path_candidates(skill_name: &str, requested_path: &str) -> Vec<String> {
    let requested = requested_path.trim_matches('/');
    let leaf = Path::new(requested)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(skill_name);
    let mut candidates = Vec::new();

    push_unique_candidate(&mut candidates, requested);
    if !requested.starts_with("skills/") {
        push_unique_candidate(&mut candidates, &format!("skills/{requested}"));
    }
    push_unique_candidate(&mut candidates, &format!("skills/{leaf}"));
    push_unique_candidate(&mut candidates, &format!("skills/{skill_name}"));
    if !requested.starts_with("skills/public/") {
        push_unique_candidate(&mut candidates, &format!("skills/public/{requested}"));
    }
    push_unique_candidate(&mut candidates, &format!("skills/public/{leaf}"));
    push_unique_candidate(&mut candidates, &format!("skills/public/{skill_name}"));
    if !requested.starts_with(".claude/skills/") {
        push_unique_candidate(&mut candidates, &format!(".claude/skills/{requested}"));
    }
    push_unique_candidate(&mut candidates, &format!(".claude/skills/{leaf}"));
    push_unique_candidate(&mut candidates, &format!(".claude/skills/{skill_name}"));

    candidates
}

pub(crate) fn push_unique_candidate(candidates: &mut Vec<String>, path: &str) {
    let path = path.trim_matches('/');
    if path.is_empty() || candidates.iter().any(|candidate| candidate == path) {
        return;
    }

    candidates.push(path.to_string());
}

pub(crate) fn github_tree_source_url(
    owner: &str,
    repo: &str,
    reference: &str,
    path: &str,
) -> String {
    format!("https://github.com/{owner}/{repo}/tree/{reference}/{path}")
}

pub(crate) fn parse_claude_marketplace_skill_candidates(
    skill_name: &str,
    response: &str,
) -> Result<Vec<RemoteSourceCandidate>> {
    let items: Vec<ClaudeMarketplaceSkill> =
        serde_json::from_str(response).map_err(|error| error.to_string())?;
    let mut exact_candidates = Vec::new();
    let mut fuzzy_candidates = Vec::new();

    for item in items {
        if !claude_marketplace_skill_is_listed(&item)
            || !claude_marketplace_skill_matches(skill_name, &item)
        {
            continue;
        }

        let Some(candidate) = claude_marketplace_skill_to_candidate(&item) else {
            continue;
        };

        if item
            .name
            .as_deref()
            .map(|name| name.eq_ignore_ascii_case(skill_name))
            .unwrap_or(false)
        {
            exact_candidates.push(candidate);
        } else {
            fuzzy_candidates.push(candidate);
        }
    }

    let candidates = if exact_candidates.is_empty() {
        fuzzy_candidates
    } else {
        exact_candidates
    };

    Ok(candidates.into_iter().take(20).collect())
}

pub(crate) fn claude_marketplace_skill_is_listed(item: &ClaudeMarketplaceSkill) -> bool {
    item.listing_status
        .as_deref()
        .map(|status| status.eq_ignore_ascii_case("listed"))
        .unwrap_or(true)
}

pub(crate) fn claude_marketplace_skill_matches(
    skill_name: &str,
    item: &ClaudeMarketplaceSkill,
) -> bool {
    let normalized_skill = skill_name.to_ascii_lowercase();
    let name = item
        .name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let path = item
        .path
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    name == normalized_skill || name.contains(&normalized_skill) || path.contains(&normalized_skill)
}

pub(crate) fn claude_marketplace_skill_to_candidate(
    item: &ClaudeMarketplaceSkill,
) -> Option<RemoteSourceCandidate> {
    let repo_label = item.repo.as_deref()?.trim();
    let path = item.path.as_deref()?.trim().trim_matches('/');
    if repo_label.is_empty() || path.is_empty() || repo_label.chars().any(char::is_whitespace) {
        return None;
    }

    let source_url = format!("https://github.com/{repo_label}/tree/main/{path}");
    let source = skillbox_github::parse_github_skill_url(&source_url).ok()?;
    let mut match_reasons = vec!["Claude Marketplace listed skill".to_string()];
    if item.installs.unwrap_or(0) > 0 {
        match_reasons.push("Claude Marketplace install signal".to_string());
    }

    Some(RemoteSourceCandidate {
        owner: source.owner,
        repo: source.repo,
        path: source.path,
        reference: source.reference,
        source_url: source.url,
        repo_url: source.repo_url,
        name: item.name.clone(),
        description: item.description.clone(),
        stars: item
            .stars
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0),
        archived: false,
        fork: false,
        updated_at: item.last_updated.clone().unwrap_or_default(),
        match_reasons,
        score: claude_marketplace_popularity_score(item),
    })
}

pub(crate) fn claude_marketplace_popularity_score(item: &ClaudeMarketplaceSkill) -> i32 {
    let install_score = item.installs.unwrap_or(0).min(1_000_000) / 5_000;
    let star_score = item.stars.unwrap_or(0).min(100_000) / 1_000;
    i32::try_from(install_score + star_score).unwrap_or(0)
}
