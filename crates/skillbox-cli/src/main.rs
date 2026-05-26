use skillbox_core::{
    default_managed_root, deploy_skill, global_runtime_roots, import_skill, managed_paths,
    scan_skill_roots, SkillKind, WorkspaceAddRequest, WorkspaceKind,
};
use skillbox_github::parse_github_skill_url;
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
        "user-skills-status" => print_json(&skillbox_core::user_skills_git_status(managed_root(
            command_args,
        ))?),
        "check-remote-updates" => print_json(&skillbox_core::check_remote_skill_updates(
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

fn help_text() -> &'static str {
    "SkillBox Rust CLI

Commands:
  skillbox paths [--managed-root <path>]
  skillbox scan [root ...] [--managed-root <path>]
  skillbox parse-github-url <github-url>
  skillbox import <source-dir> --type user|remote [--managed-root <path>]
  skillbox deploy <skill-name> --target <path> [--managed-root <path>]
  skillbox user-skills-status [--managed-root <path>]
  skillbox check-remote-updates [--managed-root <path>]
  skillbox workspaces [--managed-root <path>]
  skillbox workspace-scan [--managed-root <path>]
  skillbox workspace-add <path> --kind global|user [--managed-root <path>]
  skillbox workspace-forget <path> [--managed-root <path>]
  skillbox sync-user-skills [--remote <git-url>] [--message <msg>] [--no-push] [--managed-root <path>]
"
}
