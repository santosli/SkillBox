use crate::*;

pub const RUNTIME_PROFILE_REGISTRY_VERSION: u32 = 1;
pub const CUSTOM_SKILL_MD_PROFILE_ID: &str = "custom-skill-md";
pub const CUSTOM_SKILL_MD_ROOT_KEY: &str = "exact";

const BUILTIN_PROFILE_SPECS: [(&str, &str, &str, u32); 4] = [
    ("agents", "Agents", ".agents/skills", 10),
    ("codex", "Codex", ".codex/skills", 20),
    ("claude-code", "Claude Code", ".claude/skills", 30),
    ("cursor", "Cursor", ".cursor/skills", 40),
];

pub fn list_runtime_profiles() -> Vec<RuntimeProfile> {
    let mut profiles = BUILTIN_PROFILE_SPECS
        .iter()
        .map(|(id, display_name, relative_path, precedence)| {
            skill_md_profile(
                id,
                display_name,
                vec![RuntimeRootSpec {
                    key: "skills".to_string(),
                    relative_path: (*relative_path).to_string(),
                    scope: RuntimeRootScope::Project,
                    precedence: *precedence,
                }],
            )
        })
        .collect::<Vec<_>>();
    profiles.push(skill_md_profile(
        CUSTOM_SKILL_MD_PROFILE_ID,
        "Custom SKILL.md",
        vec![RuntimeRootSpec {
            key: CUSTOM_SKILL_MD_ROOT_KEY.to_string(),
            relative_path: String::new(),
            scope: RuntimeRootScope::Exact,
            precedence: 1000,
        }],
    ));
    profiles
}

pub fn runtime_profile(profile_id: &str) -> Option<RuntimeProfile> {
    list_runtime_profiles()
        .into_iter()
        .find(|profile| profile.id == profile_id)
}

pub(crate) fn project_runtime_roots() -> Vec<(RuntimeProfile, RuntimeRootSpec)> {
    list_runtime_profiles()
        .into_iter()
        .flat_map(|profile| {
            profile
                .roots
                .clone()
                .into_iter()
                .filter(|root| root.scope == RuntimeRootScope::Project)
                .map(move |root| (profile.clone(), root))
        })
        .collect()
}

pub(crate) fn project_runtime_parent_names() -> Vec<String> {
    project_runtime_roots()
        .into_iter()
        .filter_map(|(_, root)| {
            Path::new(&root.relative_path)
                .components()
                .next()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
        })
        .collect()
}

pub(crate) fn project_runtime_base(path: &Path) -> Option<PathBuf> {
    project_runtime_roots().into_iter().find_map(|(_, root)| {
        let suffix = Path::new(&root.relative_path);
        if !path_ends_with(path, suffix) {
            return None;
        }
        let mut base = path.to_path_buf();
        for _ in suffix.components() {
            base = base.parent()?.to_path_buf();
        }
        Some(base)
    })
}

pub(crate) fn resolve_runtime_profile_for_root(path: &Path) -> (RuntimeProfile, RuntimeRootSpec) {
    for (profile, root) in project_runtime_roots() {
        if path_ends_with(path, Path::new(&root.relative_path)) {
            return (profile, root);
        }
    }
    let profile = runtime_profile(CUSTOM_SKILL_MD_PROFILE_ID)
        .expect("custom SKILL.md profile must be registered");
    let root = profile
        .roots
        .first()
        .cloned()
        .expect("custom SKILL.md profile must define an exact root");
    (profile, root)
}

fn skill_md_profile(id: &str, display_name: &str, roots: Vec<RuntimeRootSpec>) -> RuntimeProfile {
    RuntimeProfile {
        id: id.to_string(),
        registry_version: RUNTIME_PROFILE_REGISTRY_VERSION,
        display_name: display_name.to_string(),
        format: RuntimeFormat::SkillMd,
        roots,
        deployment_modes: vec!["symlink".to_string()],
        frontmatter_policy: FrontmatterPolicy {
            supported_fields: vec![
                "name".to_string(),
                "description".to_string(),
                "version".to_string(),
            ],
            required_fields: Vec::new(),
            preserve_unknown_fields: true,
        },
    }
}

pub(crate) fn path_ends_with(path: &Path, suffix: &Path) -> bool {
    let path_components = path.components().collect::<Vec<_>>();
    let suffix_components = suffix.components().collect::<Vec<_>>();
    path_components.ends_with(&suffix_components)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_versioned_builtin_and_custom_profiles() {
        let profiles = list_runtime_profiles();
        assert_eq!(
            profiles
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "agents",
                "codex",
                "claude-code",
                "cursor",
                "custom-skill-md"
            ]
        );
        assert!(profiles
            .iter()
            .all(|profile| profile.registry_version == RUNTIME_PROFILE_REGISTRY_VERSION));
        assert!(profiles
            .iter()
            .all(|profile| profile.format == RuntimeFormat::SkillMd));
    }

    #[test]
    fn project_roots_have_explicit_deterministic_precedence() {
        let roots = project_runtime_roots();
        assert_eq!(
            roots
                .iter()
                .map(|(profile, root)| {
                    (
                        profile.id.as_str(),
                        root.relative_path.as_str(),
                        root.precedence,
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                ("agents", ".agents/skills", 10),
                ("codex", ".codex/skills", 20),
                ("claude-code", ".claude/skills", 30),
                ("cursor", ".cursor/skills", 40),
            ]
        );
    }

    #[test]
    fn root_resolution_is_component_aware_and_defaults_to_custom() {
        let (profile, root) =
            resolve_runtime_profile_for_root(Path::new("/tmp/project/.claude/skills"));
        assert_eq!(profile.id, "claude-code");
        assert_eq!(root.key, "skills");

        let (profile, root) =
            resolve_runtime_profile_for_root(Path::new("/tmp/project/.not-claude/skills"));
        assert_eq!(profile.id, CUSTOM_SKILL_MD_PROFILE_ID);
        assert_eq!(root.key, CUSTOM_SKILL_MD_ROOT_KEY);
    }

    #[test]
    fn project_base_resolution_uses_registered_root_specs() {
        assert_eq!(
            project_runtime_base(Path::new("/tmp/demo/.cursor/skills")),
            Some(PathBuf::from("/tmp/demo"))
        );
        assert_eq!(project_runtime_base(Path::new("/tmp/custom-skills")), None);
    }
}
