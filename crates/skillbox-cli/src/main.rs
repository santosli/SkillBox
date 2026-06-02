use skillbox_core::{
    default_managed_root, deploy_skill, global_runtime_roots, import_skill, managed_paths,
    scan_skill_roots, undeploy_skill, SkillKind, WorkspaceAddRequest, WorkspaceKind,
};
use skillbox_github::parse_github_skill_url;
use std::io::Read;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("skillbox: {error}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let command = args.first().map(String::as_str).unwrap_or("help");
    let command_args = &args[1..];

    match command {
        "help" | "--help" | "-h" => {
            println!("{}", help_text());
            Ok(())
        }
        "paths" => print_json(&managed_paths(managed_root(command_args))),
        "scan" => {
            let roots = positional(command_args);
            let roots = if roots.is_empty() {
                global_runtime_roots()
            } else {
                roots.into_iter().map(PathBuf::from).collect()
            };
            print_json(&scan_skill_roots(&roots)?)
        }
        "parse-github-url" => {
            let url = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox parse-github-url <github-url>".to_string())?;
            print_json(&parse_github_skill_url(&url)?)
        }
        "import" => {
            let source = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox import <source-dir> --type user|remote".to_string()
            })?;
            let kind = match option(command_args, "--type").as_deref() {
                Some("remote") => SkillKind::Remote,
                _ => SkillKind::User,
            };
            print_json(&import_skill(source, kind, managed_root(command_args))?)
        }
        "deploy" => {
            let skill_name = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox deploy <skill-name> --target <path>".to_string())?;
            let target = option(command_args, "--target")
                .ok_or_else(|| "Usage: skillbox deploy <skill-name> --target <path>".to_string())?;
            print_json(&deploy_skill(
                &skill_name,
                managed_root(command_args),
                target,
            )?)
        }
        "undeploy" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox undeploy <skill-name> --target <path>".to_string()
            })?;
            let target = option(command_args, "--target").ok_or_else(|| {
                "Usage: skillbox undeploy <skill-name> --target <path>".to_string()
            })?;
            print_json(&undeploy_skill(
                &skill_name,
                managed_root(command_args),
                target,
            )?)
        }
        "user-skills-status" => print_json(&skillbox_core::user_skills_git_status(managed_root(
            command_args,
        ))?),
        "check-remote-updates" => print_json(&skillbox_core::check_remote_skill_updates(
            managed_root(command_args),
        )?),
        "remote-source-candidates" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox remote-source-candidates <skill-name>".to_string()
            })?;
            print_json(&skillbox_core::find_remote_source_candidates(
                &skill_name,
                managed_root(command_args),
            )?)
        }
        "remote-source-preview" => {
            let values = positional(command_args);
            let skill_name = values.first().cloned().ok_or_else(|| {
                "Usage: skillbox remote-source-preview <skill-name> <github-url>".to_string()
            })?;
            let source_url = values.get(1).cloned().ok_or_else(|| {
                "Usage: skillbox remote-source-preview <skill-name> <github-url>".to_string()
            })?;
            print_json(&skillbox_core::preview_remote_source_binding(
                skillbox_core::RemoteSourceBindingRequest {
                    skill_name,
                    source_url,
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
        "bind-remote-source" => {
            let values = positional(command_args);
            let skill_name = values.first().cloned().ok_or_else(|| {
                "Usage: skillbox bind-remote-source <skill-name> <github-url>".to_string()
            })?;
            let source_url = values.get(1).cloned().ok_or_else(|| {
                "Usage: skillbox bind-remote-source <skill-name> <github-url>".to_string()
            })?;
            print_json(&skillbox_core::bind_remote_source(
                skillbox_core::BindRemoteSourceRequest {
                    skill_name,
                    source_url,
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
        "remote-versions" => {
            let skill_name = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox remote-versions <skill-name>".to_string())?;
            print_json(&skillbox_core::list_remote_skill_versions(
                &skill_name,
                managed_root(command_args),
            )?)
        }
        "remote-preview-change" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox remote-preview-change <skill-name> --action update|rollback [--to <version>]".to_string()
            })?;
            print_json(&skillbox_core::preview_remote_version_change(
                skillbox_core::RemoteVersionChangeRequest {
                    skill_name,
                    action: remote_change_action(option(command_args, "--action"))?,
                    target_version: option(command_args, "--to"),
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
        "remote-apply-change" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox remote-apply-change <skill-name> --action update|rollback --to <version>".to_string()
            })?;
            let target_version = option(command_args, "--to").ok_or_else(|| {
                "Usage: skillbox remote-apply-change <skill-name> --action update|rollback --to <version>".to_string()
            })?;
            print_json(&skillbox_core::apply_remote_version_change(
                skillbox_core::RemoteVersionChangeApplyRequest {
                    skill_name,
                    action: remote_change_action(option(command_args, "--action"))?,
                    target_version,
                    preview_id: option(command_args, "--preview-id"),
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
        "usage-record" => print_json(&skillbox_core::record_skill_usage(
            usage_record_request(command_args)?,
            managed_root(command_args),
        )?),
        "usage-hook" => {
            let agent = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox usage-hook codex|claude-code".to_string())?;
            let mut hook_input = String::new();
            let _ = std::io::stdin().read_to_string(&mut hook_input);
            let _ = skillbox_core::record_skill_usage_from_hook(
                &agent,
                &hook_input,
                managed_root(command_args),
            );
            Ok(())
        }
        "usage-hook-status" => print_json(&skillbox_core::usage_hook_statuses()?),
        "usage-hook-install" => {
            let target = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox usage-hook-install <target>".to_string())?;
            print_json(&skillbox_core::install_usage_hook(
                skillbox_core::parse_usage_hook_target(&target)?,
            )?)
        }
        "operations" => print_json(&skillbox_core::list_operations(
            skillbox_core::OperationFilter {
                entity_type: option(command_args, "--entity-type"),
                entity_name: option(command_args, "--entity-name"),
                status: option(command_args, "--status")
                    .map(|status| operation_status(&status))
                    .transpose()?,
                limit: option(command_args, "--limit")
                    .map(|limit| limit.parse::<u32>().map_err(|error| error.to_string()))
                    .transpose()?,
            },
            managed_root(command_args),
        )?),
        "workspaces" => print_json(&skillbox_core::list_workspaces(managed_root(command_args))?),
        "workspace-scan" => {
            print_json(&skillbox_core::scan_workspaces(managed_root(command_args))?)
        }
        "workspace-add" => {
            let path = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox workspace-add <path> --kind global|user".to_string()
            })?;
            let kind = workspace_kind(command_args)?;
            print_json(&skillbox_core::add_workspace(
                WorkspaceAddRequest {
                    path: PathBuf::from(path),
                    kind,
                },
                managed_root(command_args),
            )?)
        }
        "workspace-forget" => {
            let path = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox workspace-forget <path>".to_string())?;
            print_json(&skillbox_core::forget_workspace(
                PathBuf::from(path),
                managed_root(command_args),
            )?)
        }
        "sync-user-skills" => {
            let request = skillbox_core::UserSkillsSyncRequest {
                remote_url: option(command_args, "--remote"),
                commit_message: option(command_args, "--message"),
                push: !has_flag(command_args, "--no-push"),
                selected_paths: None,
            };
            print_json(&skillbox_core::sync_user_skills_git(
                request,
                managed_root(command_args),
            )?)
        }
        other => Err(format!("Unknown command: {other}")),
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), String> {
    let output = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    println!("{output}");
    Ok(())
}

fn managed_root(args: &[String]) -> PathBuf {
    option(args, "--managed-root")
        .map(PathBuf::from)
        .unwrap_or_else(default_managed_root)
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn positional(args: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let value = &args[index];
        if value.starts_with("--") {
            if args
                .get(index + 1)
                .is_some_and(|next| !next.starts_with("--"))
            {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        result.push(value.clone());
        index += 1;
    }
    result
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn workspace_kind(args: &[String]) -> Result<WorkspaceKind, String> {
    match option(args, "--kind").as_deref() {
        Some("global") => Ok(WorkspaceKind::Global),
        Some("user") | None => Ok(WorkspaceKind::User),
        Some(other) => Err(format!("Invalid workspace kind: {other}")),
    }
}

fn remote_change_action(
    value: Option<String>,
) -> Result<skillbox_core::RemoteVersionChangeAction, String> {
    match value.as_deref() {
        Some("update") => Ok(skillbox_core::RemoteVersionChangeAction::Update),
        Some("rollback") => Ok(skillbox_core::RemoteVersionChangeAction::Rollback),
        _ => Err("Use --action update|rollback".to_string()),
    }
}

fn operation_status(value: &str) -> Result<skillbox_core::OperationStatus, String> {
    match value {
        "started" => Ok(skillbox_core::OperationStatus::Started),
        "succeeded" => Ok(skillbox_core::OperationStatus::Succeeded),
        "failed" => Ok(skillbox_core::OperationStatus::Failed),
        "cancelled" => Ok(skillbox_core::OperationStatus::Cancelled),
        other => Err(format!("Invalid operation status: {other}")),
    }
}

fn usage_record_request(args: &[String]) -> Result<skillbox_core::RecordSkillUsageRequest, String> {
    let skill_name = option(args, "--skill").ok_or_else(|| {
        "Usage: skillbox usage-record --skill <name> --agent <id> --runtime-root <path>".to_string()
    })?;
    let agent_id = option(args, "--agent").ok_or_else(|| {
        "Usage: skillbox usage-record --skill <name> --agent <id> --runtime-root <path>".to_string()
    })?;
    let runtime_root = option(args, "--runtime-root").ok_or_else(|| {
        "Usage: skillbox usage-record --skill <name> --agent <id> --runtime-root <path>".to_string()
    })?;
    let metadata = option(args, "--metadata-json")
        .map(|value| {
            serde_json::from_str(&value)
                .map_err(|error| format!("Invalid --metadata-json: {error}"))
        })
        .transpose()?;

    Ok(skillbox_core::RecordSkillUsageRequest {
        skill_name,
        agent_id,
        runtime_root: PathBuf::from(runtime_root),
        event_id: option(args, "--event-id"),
        used_at: option(args, "--used-at"),
        metadata,
    })
}

fn help_text() -> &'static str {
    "SkillBox Rust CLI

Commands:
  skillbox paths [--managed-root <path>]
  skillbox scan [root ...] [--managed-root <path>]
  skillbox parse-github-url <github-url>
  skillbox import <source-dir> --type user|remote [--managed-root <path>]
  skillbox deploy <skill-name> --target <path> [--managed-root <path>]
  skillbox undeploy <skill-name> --target <path> [--managed-root <path>]
  skillbox user-skills-status [--managed-root <path>]
  skillbox check-remote-updates [--managed-root <path>]
  skillbox remote-source-candidates <skill-name> [--managed-root <path>]
  skillbox remote-source-preview <skill-name> <github-url> [--managed-root <path>]
  skillbox bind-remote-source <skill-name> <github-url> [--managed-root <path>]
  skillbox remote-versions <skill-name> [--managed-root <path>]
  skillbox remote-preview-change <skill-name> --action update|rollback [--to <version>] [--managed-root <path>]
  skillbox remote-apply-change <skill-name> --action update|rollback --to <version> [--preview-id <id>] [--managed-root <path>]
  skillbox usage-record --skill <name> --agent <id> --runtime-root <path> [--event-id <id>] [--used-at <rfc3339>] [--metadata-json <json>] [--managed-root <path>]
  skillbox usage-hook codex|claude-code [--managed-root <path>]
  skillbox usage-hook-status
  skillbox usage-hook-install <target>
  skillbox operations [--entity-type <type>] [--entity-name <name>] [--status started|succeeded|failed|cancelled] [--limit <n>] [--managed-root <path>]
  skillbox workspaces [--managed-root <path>]
  skillbox workspace-scan [--managed-root <path>]
  skillbox workspace-add <path> --kind global|user [--managed-root <path>]
  skillbox workspace-forget <path> [--managed-root <path>]
  skillbox sync-user-skills [--remote <git-url>] [--message <msg>] [--no-push] [--managed-root <path>]
"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_record_request_parses_required_and_optional_fields() {
        let args = vec![
            "--skill".to_string(),
            "grill-me".to_string(),
            "--agent".to_string(),
            "codex".to_string(),
            "--runtime-root".to_string(),
            "/Users/santos/.codex/skills".to_string(),
            "--event-id".to_string(),
            "codex-run-1".to_string(),
            "--used-at".to_string(),
            "2026-06-02T10:15:00Z".to_string(),
            "--metadata-json".to_string(),
            r#"{"source":"codex-app"}"#.to_string(),
        ];

        let request = usage_record_request(&args).unwrap();

        assert_eq!(request.skill_name, "grill-me");
        assert_eq!(request.agent_id, "codex");
        assert_eq!(
            request.runtime_root,
            PathBuf::from("/Users/santos/.codex/skills")
        );
        assert_eq!(request.event_id.as_deref(), Some("codex-run-1"));
        assert_eq!(request.used_at.as_deref(), Some("2026-06-02T10:15:00Z"));
        assert_eq!(
            request.metadata.as_ref().unwrap()["source"].as_str(),
            Some("codex-app")
        );
    }

    #[test]
    fn usage_record_request_rejects_invalid_metadata_json() {
        let args = vec![
            "--skill".to_string(),
            "grill-me".to_string(),
            "--agent".to_string(),
            "codex".to_string(),
            "--runtime-root".to_string(),
            "/Users/santos/.codex/skills".to_string(),
            "--metadata-json".to_string(),
            "{broken".to_string(),
        ];

        let error = usage_record_request(&args).unwrap_err();

        assert!(error.contains("--metadata-json"));
    }

    #[test]
    fn usage_hook_targets_are_supported_by_cli_help() {
        assert!(matches!(
            skillbox_core::parse_usage_hook_target("codex-cli").unwrap(),
            skillbox_core::UsageHookTarget::CodexCli
        ));
        assert!(matches!(
            skillbox_core::parse_usage_hook_target("claude-code").unwrap(),
            skillbox_core::UsageHookTarget::ClaudeCodeCli
        ));
        assert!(help_text().contains("skillbox usage-hook codex|claude-code"));
        assert!(help_text().contains("skillbox usage-hook-install <target>"));
    }
}
