use skillbox_core::{
    default_managed_root, ensure_managed_layout, global_runtime_roots, import_skill, managed_paths,
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
        "version" | "--version" | "-V" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "init" => print_json(&ensure_managed_layout(managed_root(command_args))?),
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
        "runtime-profiles" => print_json(&skillbox_core::list_runtime_profiles()),
        "install" => {
            let source_url = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox install <github-url> --preview-id <id> [--target <path>]"
                    .to_string()
            })?;
            let preview_id = option(command_args, "--preview-id").ok_or_else(|| {
                "Remote install preview is required. Run `skillbox install-preview <github-url>` first, then pass --preview-id <id>.".to_string()
            })?;
            print_json(&skillbox_core::install_github_remote_skill(
                skillbox_core::InstallGithubRemoteSkillRequest {
                    source_url,
                    target_root: option(command_args, "--target").map(PathBuf::from),
                    preview_id: Some(preview_id),
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
        "install-preview" => {
            let source_url = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox install-preview <github-url> [--target <path>]".to_string()
            })?;
            print_json(&skillbox_core::preview_github_remote_skill_install(
                skillbox_core::PreviewGithubRemoteSkillInstallRequest {
                    source_url,
                    target_root: option(command_args, "--target").map(PathBuf::from),
                },
                managed_root(command_args),
            )?)
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
        "deploy-preview" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox deploy-preview <skill-name> --target <path>".to_string()
            })?;
            let target = option(command_args, "--target").ok_or_else(|| {
                "Usage: skillbox deploy-preview <skill-name> --target <path>".to_string()
            })?;
            print_json(&skillbox_core::preview_skill_deployment(
                skillbox_core::DeploymentCompatibilityPreviewRequest {
                    skill_name,
                    target_root: PathBuf::from(target),
                },
                managed_root(command_args),
            )?)
        }
        "deploy" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox deploy <skill-name> --target <path> --preview-id <id>".to_string()
            })?;
            let target = option(command_args, "--target").ok_or_else(|| {
                "Usage: skillbox deploy <skill-name> --target <path> --preview-id <id>".to_string()
            })?;
            let preview_id = option(command_args, "--preview-id").ok_or_else(|| {
                "Deployment compatibility preview is required. Run `skillbox deploy-preview <skill-name> --target <path>` first."
                    .to_string()
            })?;
            print_json(&skillbox_core::apply_skill_deployment(
                skillbox_core::DeploymentCompatibilityApplyRequest {
                    skill_name,
                    target_root: PathBuf::from(target),
                    preview_id,
                    confirm_warnings: has_flag(command_args, "--confirm-warnings"),
                },
                managed_root(command_args),
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
        "delete-preview" => {
            let skill_name = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox delete-preview <skill-name>".to_string())?;
            print_json(&skillbox_core::preview_delete_skill(
                &skill_name,
                managed_root(command_args),
            )?)
        }
        "delete" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox delete <skill-name> --preview-id <id> --confirm <skill-name>"
                    .to_string()
            })?;
            let preview_id = option(command_args, "--preview-id").ok_or_else(|| {
                "Deletion preview is required. Run `skillbox delete-preview <skill-name>` first."
                    .to_string()
            })?;
            let confirmed_skill_name = option(command_args, "--confirm")
                .ok_or_else(|| "Confirm deletion by passing --confirm <skill-name>.".to_string())?;
            print_json(&skillbox_core::delete_skill(
                skillbox_core::DeleteSkillRequest {
                    skill_name,
                    preview_id,
                    confirmed_skill_name,
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
        "user-skills-status" => print_json(&skillbox_core::user_skills_git_status(managed_root(
            command_args,
        ))?),
        "check-remote-updates" | "check-updates" => {
            let skill_name = positional(command_args).into_iter().next();
            if let Some(skill_name) = skill_name {
                print_json(&skillbox_core::check_remote_skill_update(
                    managed_root(command_args),
                    &skill_name,
                )?)
            } else {
                print_json(&skillbox_core::check_remote_skill_updates(managed_root(
                    command_args,
                ))?)
            }
        }
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
        "rollback" => {
            let skill_name = positional(command_args).into_iter().next().ok_or_else(|| {
                "Usage: skillbox rollback <skill-name> --to <version>".to_string()
            })?;
            let target_version = option(command_args, "--to").ok_or_else(|| {
                "Usage: skillbox rollback <skill-name> --to <version>".to_string()
            })?;
            print_json(&skillbox_core::apply_remote_version_change(
                skillbox_core::RemoteVersionChangeApplyRequest {
                    skill_name,
                    action: skillbox_core::RemoteVersionChangeAction::Rollback,
                    target_version,
                    preview_id: None,
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
        "usage-record" => print_json(&skillbox_core::record_skill_usage(
            usage_record_request(command_args)?,
            managed_root(command_args),
        )?),
        "usage-rankings" => print_json(&skillbox_core::list_skill_usage_rankings(
            usage_ranking_request(command_args)?,
            managed_root(command_args),
        )?),
        "usage-backfill-codex" => print_json(&skillbox_core::backfill_codex_session_usage(
            skillbox_core::BackfillCodexSessionUsageRequest {
                include_archived: has_flag(command_args, "--include-archived"),
                sessions_root: option(command_args, "--sessions-root").map(PathBuf::from),
                archived_sessions_root: option(command_args, "--archived-sessions-root")
                    .map(PathBuf::from),
            },
            managed_root(command_args),
        )?),
        "usage-backfill-claude-code" => {
            print_json(&skillbox_core::backfill_claude_code_session_usage(
                skillbox_core::BackfillClaudeCodeSessionUsageRequest {
                    projects_root: option(command_args, "--projects-root").map(PathBuf::from),
                },
                managed_root(command_args),
            )?)
        }
        "usage-backfill-cursor" => print_json(&skillbox_core::backfill_cursor_session_usage(
            skillbox_core::BackfillCursorSessionUsageRequest {
                database_path: option(command_args, "--database-path").map(PathBuf::from),
            },
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
        "doctor" => print_json(&skillbox_core::run_doctor(
            skillbox_core::DoctorRequest {
                repair_preview: has_flag(command_args, "--repair-preview"),
            },
            managed_root(command_args),
        )?),
        "doctor-clean-stale-deployments" => print_json(
            &skillbox_core::repair_stale_deployment_records(managed_root(command_args))?,
        ),
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
        "import-records" => print_json(&skillbox_core::list_import_records(
            skillbox_core::ImportRecordFilter {
                skill_name: option(command_args, "--skill"),
            },
            managed_root(command_args),
        )?),
        "revert-import" => {
            let import_record_id = positional(command_args)
                .into_iter()
                .next()
                .ok_or_else(|| "Usage: skillbox revert-import <import-record-id>".to_string())?;
            print_json(&skillbox_core::revert_import(
                skillbox_core::RevertImportRequest {
                    import_record_id,
                    actor: "cli".to_string(),
                },
                managed_root(command_args),
            )?)
        }
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
        prompt_excerpt: option(args, "--prompt-excerpt"),
        metadata,
    })
}

fn usage_ranking_request(
    args: &[String],
) -> Result<skillbox_core::SkillUsageRankingRequest, String> {
    let range = match option(args, "--range").as_deref() {
        None | Some("30d") => skillbox_core::SkillUsageRankingRange::Last30Days,
        Some("7d") => skillbox_core::SkillUsageRankingRange::Last7Days,
        Some("all") => skillbox_core::SkillUsageRankingRange::AllTime,
        Some(other) => {
            return Err(format!(
                "Invalid usage ranking range: {other}. Use 7d, 30d, or all."
            ))
        }
    };
    let skill_type = match option(args, "--type").as_deref() {
        None => None,
        Some("user") => Some(skillbox_core::SkillUsageRankingSkillType::User),
        Some("remote") => Some(skillbox_core::SkillUsageRankingSkillType::Remote),
        Some("system") => Some(skillbox_core::SkillUsageRankingSkillType::System),
        Some(other) => {
            return Err(format!(
                "Invalid usage ranking skill type: {other}. Use user, remote, or system."
            ))
        }
    };
    Ok(skillbox_core::SkillUsageRankingRequest {
        range,
        skill_type,
        agent_id: option(args, "--agent"),
        workspace_root: option(args, "--workspace").map(PathBuf::from),
        include_unmanaged: has_flag(args, "--include-unmanaged"),
    })
}

fn help_text() -> &'static str {
    "SkillBox Rust CLI

Commands:
  skillbox init [--managed-root <path>]
  skillbox version
  skillbox paths [--managed-root <path>]
  skillbox scan [root ...] [--managed-root <path>]
  skillbox parse-github-url <github-url>
  skillbox runtime-profiles
  skillbox install-preview <github-url> [--target <path>] [--managed-root <path>]
  skillbox install <github-url> --preview-id <id> [--target <path>] [--managed-root <path>]
  skillbox import <source-dir> --type user|remote [--managed-root <path>]
  skillbox deploy-preview <skill-name> --target <path> [--managed-root <path>]
  skillbox deploy <skill-name> --target <path> --preview-id <id> [--confirm-warnings] [--managed-root <path>]
  skillbox undeploy <skill-name> --target <path> [--managed-root <path>]
  skillbox delete-preview <skill-name> [--managed-root <path>]
  skillbox delete <skill-name> --preview-id <id> --confirm <skill-name> [--managed-root <path>]
  skillbox user-skills-status [--managed-root <path>]
  skillbox check-remote-updates [skill-name] [--managed-root <path>]
  skillbox check-updates [skill-name] [--managed-root <path>]
  skillbox remote-source-candidates <skill-name> [--managed-root <path>]
  skillbox remote-source-preview <skill-name> <github-url> [--managed-root <path>]
  skillbox bind-remote-source <skill-name> <github-url> [--managed-root <path>]
  skillbox remote-versions <skill-name> [--managed-root <path>]
  skillbox remote-preview-change <skill-name> --action update|rollback [--to <version>] [--managed-root <path>]
  skillbox remote-apply-change <skill-name> --action update|rollback --to <version> [--preview-id <id>] [--managed-root <path>]
  skillbox rollback <skill-name> --to <version> [--managed-root <path>]
  skillbox usage-record --skill <name> --agent <id> --runtime-root <path> [--event-id <id>] [--used-at <rfc3339>] [--prompt-excerpt <text>] [--metadata-json <json>] [--managed-root <path>]
  skillbox usage-rankings [--range 7d|30d|all] [--type user|remote|system] [--agent <id>] [--workspace <path>] [--include-unmanaged] [--managed-root <path>]
  skillbox usage-backfill-codex [--include-archived] [--sessions-root <path>] [--archived-sessions-root <path>] [--managed-root <path>]
  skillbox usage-backfill-claude-code [--projects-root <path>] [--managed-root <path>]
  skillbox usage-backfill-cursor [--database-path <path>] [--managed-root <path>]
  skillbox usage-hook codex|claude-code [--managed-root <path>]
  skillbox usage-hook-status
  skillbox usage-hook-install <target>
  skillbox doctor [--repair-preview] [--managed-root <path>]
  skillbox doctor-clean-stale-deployments [--managed-root <path>]
  skillbox operations [--entity-type <type>] [--entity-name <name>] [--status started|succeeded|failed|cancelled] [--limit <n>] [--managed-root <path>]
  skillbox import-records [--skill <name>] [--managed-root <path>]
  skillbox revert-import <import-record-id> [--managed-root <path>]
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
            "/Users/example/.codex/skills".to_string(),
            "--event-id".to_string(),
            "codex-run-1".to_string(),
            "--used-at".to_string(),
            "2026-06-02T10:15:00Z".to_string(),
            "--prompt-excerpt".to_string(),
            "Use grill-me on the plan".to_string(),
            "--metadata-json".to_string(),
            r#"{"source":"codex-app"}"#.to_string(),
        ];

        let request = usage_record_request(&args).unwrap();

        assert_eq!(request.skill_name, "grill-me");
        assert_eq!(request.agent_id, "codex");
        assert_eq!(
            request.runtime_root,
            PathBuf::from("/Users/example/.codex/skills")
        );
        assert_eq!(request.event_id.as_deref(), Some("codex-run-1"));
        assert_eq!(request.used_at.as_deref(), Some("2026-06-02T10:15:00Z"));
        assert_eq!(
            request.prompt_excerpt.as_deref(),
            Some("Use grill-me on the plan")
        );
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
            "/Users/example/.codex/skills".to_string(),
            "--metadata-json".to_string(),
            "{broken".to_string(),
        ];

        let error = usage_record_request(&args).unwrap_err();

        assert!(error.contains("--metadata-json"));
    }

    #[test]
    fn usage_ranking_request_defaults_to_managed_last_thirty_days() {
        let request = usage_ranking_request(&[]).unwrap();

        assert_eq!(
            request.range,
            skillbox_core::SkillUsageRankingRange::Last30Days
        );
        assert_eq!(request.skill_type, None);
        assert_eq!(request.agent_id, None);
        assert_eq!(request.workspace_root, None);
        assert!(!request.include_unmanaged);
    }

    #[test]
    fn usage_ranking_request_parses_filters_and_unmanaged_scope() {
        let args = vec![
            "--range".to_string(),
            "7d".to_string(),
            "--type".to_string(),
            "remote".to_string(),
            "--agent".to_string(),
            "codex".to_string(),
            "--workspace".to_string(),
            "/Users/example/.codex/skills".to_string(),
            "--include-unmanaged".to_string(),
        ];

        let request = usage_ranking_request(&args).unwrap();

        assert_eq!(
            request.range,
            skillbox_core::SkillUsageRankingRange::Last7Days
        );
        assert_eq!(
            request.skill_type,
            Some(skillbox_core::SkillUsageRankingSkillType::Remote)
        );
        assert_eq!(request.agent_id.as_deref(), Some("codex"));
        assert_eq!(
            request.workspace_root,
            Some(PathBuf::from("/Users/example/.codex/skills"))
        );
        assert!(request.include_unmanaged);
    }

    #[test]
    fn usage_ranking_request_rejects_invalid_range() {
        let error =
            usage_ranking_request(&["--range".to_string(), "week".to_string()]).unwrap_err();

        assert!(error.contains("7d, 30d, or all"));
        assert!(help_text().contains("skillbox usage-rankings"));
        assert!(help_text().contains("skillbox usage-backfill-codex"));
        assert!(help_text().contains("skillbox usage-backfill-claude-code"));
        assert!(help_text().contains("skillbox usage-backfill-cursor"));
    }

    #[test]
    fn usage_ranking_request_rejects_invalid_skill_type() {
        let error =
            usage_ranking_request(&["--type".to_string(), "managed".to_string()]).unwrap_err();

        assert!(error.contains("Invalid usage ranking skill type: managed"));
        assert!(error.contains("user, remote, or system"));
    }

    #[test]
    fn usage_rankings_command_routes_to_core() {
        let managed_root = temp_dir("cli-usage-rankings").join("SkillBox");

        run(vec![
            "usage-rankings".to_string(),
            "--range".to_string(),
            "all".to_string(),
            "--managed-root".to_string(),
            managed_root.to_string_lossy().to_string(),
        ])
        .unwrap();

        assert!(managed_root.join("skillbox.sqlite").is_file());
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

    #[test]
    fn help_lists_legacy_node_cli_compatibility_commands() {
        let help = help_text();

        assert!(help.contains("skillbox init [--managed-root <path>]"));
        assert!(help.contains("skillbox runtime-profiles"));
        assert!(help.contains("skillbox deploy-preview <skill-name>"));
        assert!(help.contains("skillbox deploy <skill-name> --target <path> --preview-id <id>"));
        assert!(help.contains("skillbox install-preview <github-url>"));
        assert!(help.contains("skillbox install <github-url> --preview-id <id>"));
        assert!(help.contains("skillbox check-updates [skill-name]"));
        assert!(help.contains("skillbox rollback <skill-name> --to <version>"));
        assert!(help.contains("skillbox version"));
    }

    #[test]
    fn help_lists_import_record_commands() {
        let help = help_text();

        assert!(help.contains("skillbox import-records [--skill <name>]"));
        assert!(help.contains("skillbox revert-import <import-record-id>"));
    }

    #[test]
    fn delete_commands_require_review_and_route_to_core() {
        let root = temp_dir("cli-delete-skill");
        let managed_root = root.join("SkillBox");
        let source = root.join("source").join("demo");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\n",
        )
        .unwrap();
        skillbox_core::import_skill(&source, SkillKind::User, &managed_root).unwrap();
        let preview = skillbox_core::preview_delete_skill("demo", &managed_root).unwrap();

        run(vec![
            "delete-preview".to_string(),
            "demo".to_string(),
            "--managed-root".to_string(),
            managed_root.to_string_lossy().to_string(),
        ])
        .unwrap();
        run(vec![
            "delete".to_string(),
            "demo".to_string(),
            "--preview-id".to_string(),
            preview.preview_id,
            "--confirm".to_string(),
            "demo".to_string(),
            "--managed-root".to_string(),
            managed_root.to_string_lossy().to_string(),
        ])
        .unwrap();

        assert!(skillbox_core::managed_state(&managed_root)
            .unwrap()
            .skills
            .is_empty());
    }

    #[test]
    fn doctor_command_routes_to_read_only_health_check() {
        let root = temp_dir("cli-doctor").join("SkillBox");

        assert!(help_text().contains("skillbox doctor [--repair-preview]"));
        run(vec![
            "doctor".to_string(),
            "--repair-preview".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap();

        assert!(root.join("skillbox.sqlite").is_file());
        assert!(root.join("user-skills").is_dir());
        assert!(root.join("remote-skills").is_dir());
    }

    #[test]
    fn doctor_cleanup_command_routes_to_safe_stale_record_repair() {
        let root = temp_dir("cli-doctor-cleanup").join("SkillBox");

        assert!(help_text().contains("skillbox doctor-clean-stale-deployments"));
        run(vec![
            "doctor-clean-stale-deployments".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap();

        assert!(root.join("skillbox.sqlite").is_file());
    }

    #[test]
    fn version_commands_are_supported() {
        assert!(run(vec!["version".to_string()]).is_ok());
        assert!(run(vec!["--version".to_string()]).is_ok());
    }

    #[test]
    fn init_creates_managed_layout() {
        let root = temp_dir("cli-init").join("SkillBox");

        run(vec![
            "init".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap();

        assert!(root.join("user-skills").exists());
        assert!(root.join("remote-skills").exists());
        assert!(root.join("skillbox.sqlite").exists());
    }

    #[test]
    fn check_updates_legacy_alias_is_supported() {
        let root = temp_dir("cli-check-updates").join("SkillBox");

        run(vec![
            "check-updates".to_string(),
            "missing-skill".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap();
    }

    #[test]
    fn rollback_legacy_alias_routes_to_remote_apply() {
        let root = temp_dir("cli-rollback").join("SkillBox");

        let error = run(vec![
            "rollback".to_string(),
            "missing-skill".to_string(),
            "--to".to_string(),
            "1234".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap_err();

        assert!(!error.contains("Unknown command"));
    }

    #[test]
    fn import_record_commands_route_to_core() {
        let root = temp_dir("cli-import-records").join("SkillBox");

        run(vec![
            "import-records".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap();

        let error = run(vec![
            "revert-import".to_string(),
            "missing-record".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap_err();

        assert!(!error.contains("Unknown command"));
    }

    #[test]
    fn install_requires_preview_id() {
        let root = temp_dir("cli-install").join("SkillBox");

        let error = run(vec![
            "install".to_string(),
            "https://example.com/acme/repo/tree/main/skills/demo".to_string(),
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap_err();

        assert!(error.contains("Remote install preview is required"));
    }

    #[test]
    fn install_preview_command_routes_to_core_without_writing_store() {
        let root = temp_dir("cli-install-preview").join("SkillBox");
        let remote = bare_remote_with_skill_content("cli-install-preview-origin", "demo");
        let _rewrite = github_repo_rewrite("acme", "cli-install-preview", &remote);
        let source_url = github_source_url("acme", "cli-install-preview", "demo");

        run(vec![
            "install-preview".to_string(),
            source_url,
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap();

        assert!(!root.join("remote-skills").join("demo").exists());
    }

    #[test]
    fn install_accepts_valid_preview_id() {
        let root = temp_dir("cli-install-valid-preview").join("SkillBox");
        let remote = bare_remote_with_skill_content("cli-install-valid-preview-origin", "demo");
        let _rewrite = github_repo_rewrite("acme", "cli-install-valid-preview", &remote);
        let source_url = github_source_url("acme", "cli-install-valid-preview", "demo");
        let preview = skillbox_core::preview_github_remote_skill_install(
            skillbox_core::PreviewGithubRemoteSkillInstallRequest {
                source_url: source_url.clone(),
                target_root: None,
            },
            &root,
        )
        .unwrap();

        run(vec![
            "install".to_string(),
            source_url,
            "--preview-id".to_string(),
            preview.preview_id,
            "--managed-root".to_string(),
            root.to_string_lossy().to_string(),
        ])
        .unwrap();

        assert!(root
            .join("remote-skills")
            .join("demo")
            .join("current")
            .exists());
    }

    #[test]
    fn runtime_profiles_and_preview_confirmed_deploy_commands_route_to_core() {
        let root = temp_dir("cli-runtime-profile-deploy");
        let managed_root = root.join("SkillBox");
        let source = root.join("source/demo");
        let target_root = root.join("project/.cursor/skills");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&target_root).unwrap();
        std::fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\n",
        )
        .unwrap();
        skillbox_core::import_skill(&source, SkillKind::User, &managed_root).unwrap();
        skillbox_core::add_workspace(
            skillbox_core::WorkspaceAddRequest {
                path: target_root.clone(),
                kind: skillbox_core::WorkspaceKind::User,
            },
            &managed_root,
        )
        .unwrap();

        run(vec!["runtime-profiles".to_string()]).unwrap();
        run(vec![
            "deploy-preview".to_string(),
            "demo".to_string(),
            "--target".to_string(),
            target_root.to_string_lossy().to_string(),
            "--managed-root".to_string(),
            managed_root.to_string_lossy().to_string(),
        ])
        .unwrap();
        let missing_preview = run(vec![
            "deploy".to_string(),
            "demo".to_string(),
            "--target".to_string(),
            target_root.to_string_lossy().to_string(),
            "--managed-root".to_string(),
            managed_root.to_string_lossy().to_string(),
        ])
        .unwrap_err();
        assert!(missing_preview.contains("Deployment compatibility preview is required"));

        let preview = skillbox_core::preview_skill_deployment(
            skillbox_core::DeploymentCompatibilityPreviewRequest {
                skill_name: "demo".to_string(),
                target_root: target_root.clone(),
            },
            &managed_root,
        )
        .unwrap();
        run(vec![
            "deploy".to_string(),
            "demo".to_string(),
            "--target".to_string(),
            target_root.to_string_lossy().to_string(),
            "--preview-id".to_string(),
            preview.preview_id,
            "--managed-root".to_string(),
            managed_root.to_string_lossy().to_string(),
        ])
        .unwrap();
        assert!(std::fs::symlink_metadata(target_root.join("demo"))
            .unwrap()
            .file_type()
            .is_symlink());
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("skillbox-cli-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn bare_remote_with_skill_content(label: &str, skill_name: &str) -> PathBuf {
        let remote = temp_dir(label).join("remote.git");
        run_git(
            remote.parent().unwrap(),
            &["init", "--bare", remote.to_str().unwrap()],
        );
        let work = temp_dir(&format!("{label}-work"));
        run_git(&work, &["init", "-b", "main"]);
        let skill_dir = work.join("skills").join(skill_name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {skill_name}\ndescription: Demo skill\n---\n\n# {skill_name}\n"),
        )
        .unwrap();
        run_git(&work, &["add", "."]);
        run_git(
            &work,
            &[
                "-c",
                "user.name=SkillBox",
                "-c",
                "user.email=skillbox@example.invalid",
                "commit",
                "-m",
                "Add skill",
            ],
        );
        run_git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        run_git(&work, &["push", "-u", "origin", "main"]);
        remote
    }

    fn github_source_url(owner: &str, repo: &str, skill_name: &str) -> String {
        format!("https://github.com/{owner}/{repo}/tree/main/skills/{skill_name}")
    }

    fn run_git(repo: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    static GIT_CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct GitConfigRewriteGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl Drop for GitConfigRewriteGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn github_repo_rewrite(
        owner: &str,
        repo: &str,
        remote: &std::path::Path,
    ) -> GitConfigRewriteGuard {
        let lock = GIT_CONFIG_LOCK.lock().unwrap();
        let keys = ["GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0"];
        let previous = keys
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect::<Vec<_>>();

        std::env::set_var("GIT_CONFIG_COUNT", "1");
        std::env::set_var(
            "GIT_CONFIG_KEY_0",
            format!("url.file://{}.insteadOf", remote.display()),
        );
        std::env::set_var(
            "GIT_CONFIG_VALUE_0",
            format!("https://github.com/{owner}/{repo}.git"),
        );

        GitConfigRewriteGuard {
            _lock: lock,
            previous,
        }
    }
}
