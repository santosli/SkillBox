use super::*;
use crate::test_support::*;
use std::fs;
use std::sync::{Arc, Barrier};

#[test]
fn github_collection_update_apply_advances_sha_and_rollback_restores_previous() {
    let root = temp_dir("github-collection-update-apply");
    let managed_root = root.join("SkillBox");
    let (remote, work) = bare_remote_with_multiple_skill_content(
        "github-collection-update-apply-origin",
        &["alpha", "beta"],
    );
    let _rewrite = github_repo_rewrite("acme", "github-collection-update-apply", &remote);
    let source_url = "https://github.com/acme/github-collection-update-apply/tree/main";
    let first = preview_github_skill_collection(
        PreviewGithubSkillCollectionRequest {
            source_url: source_url.to_string(),
        },
        &managed_root,
    )
    .unwrap();
    let from_sha = first.collection.reviewed_head_sha.clone().unwrap();
    let alpha = first
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let beta = first
        .collection
        .children
        .iter()
        .find(|child| child.name == "beta")
        .unwrap()
        .clone();
    apply_github_skill_collection(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: first.collection.id.clone(),
            preview_id: first.collection.preview_id,
            selections: vec![
                github_collection_child_selection(&alpha, SkillKind::User),
                github_collection_child_selection(&beta, SkillKind::Remote),
            ],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    fs::write(
        work.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Updated alpha\n---\n\n# Alpha updated\n",
    )
    .unwrap();
    fs::remove_dir_all(work.join("skills/beta")).unwrap();
    make_skill(&work.join("skills/gamma"), "gamma", "Gamma");
    commit_and_push_collection(&work, "Advance collection");

    let preview = github_update_preview(source_url, &managed_root);
    let to_sha = preview.to_sha.clone();
    let updated_alpha = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "alpha")
        .unwrap()
        .clone();
    let added_gamma = preview
        .collection
        .children
        .iter()
        .find(|child| child.name == "gamma")
        .unwrap()
        .clone();
    let result = apply_github_skill_collection_update(
        GithubSkillCollectionApplyRequest {
            source_url: source_url.to_string(),
            collection_id: preview.collection_id.clone(),
            preview_id: preview.preview_id.clone(),
            selections: vec![
                github_collection_child_selection(&updated_alpha, SkillKind::User),
                github_collection_child_selection(&added_gamma, SkillKind::User),
            ],
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();

    assert_eq!(
        result.collection.reviewed_head_sha.as_deref(),
        Some(to_sha.as_str())
    );
    assert_eq!(
        result.collection.previous_reviewed_head_sha.as_deref(),
        Some(from_sha.as_str())
    );
    let names: Vec<_> = result
        .collection
        .members
        .iter()
        .map(|member| member.managed_skill_name.as_str())
        .collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"gamma"));
    assert!(!names.contains(&"beta"));
    let paths = managed_paths(&managed_root);
    let alpha_body = fs::read_to_string(paths.user_skills_root.join("alpha/SKILL.md")).unwrap();
    assert!(alpha_body.contains("Alpha updated"));
    assert!(paths
        .remote_skills_root
        .join("beta/current/SKILL.md")
        .exists());
    assert!(paths.user_skills_root.join("gamma/SKILL.md").exists());
    let connection = open_database(&paths.database_path).unwrap();
    let deployment_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM deployments", [], |row| row.get(0))
        .unwrap();
    assert_eq!(deployment_count, 0);

    let rollback_preview = preview_github_skill_collection_rollback(
        GithubCollectionRollbackRequest {
            collection_id: result.collection.id.clone(),
            preview_id: String::new(),
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(rollback_preview.to_sha, from_sha);
    assert!(rollback_preview
        .leaving_skill_names
        .contains(&"gamma".to_string()));
    let rolled = apply_github_skill_collection_rollback(
        GithubCollectionRollbackRequest {
            collection_id: rollback_preview.collection_id,
            preview_id: rollback_preview.preview_id,
            actor: "test".to_string(),
        },
        &managed_root,
    )
    .unwrap();
    assert_eq!(
        rolled.collection.reviewed_head_sha.as_deref(),
        Some(from_sha.as_str())
    );
    assert_eq!(rolled.collection.previous_reviewed_head_sha, None);
    let restored_alpha = fs::read_to_string(paths.user_skills_root.join("alpha/SKILL.md")).unwrap();
    assert!(restored_alpha.contains("alpha skill"));
    let restored_names: Vec<_> = rolled
        .collection
        .members
        .iter()
        .map(|member| member.managed_skill_name.as_str())
        .collect();
    assert!(restored_names.contains(&"alpha"));
    assert!(restored_names.contains(&"beta"));
    assert!(!restored_names.contains(&"gamma"));
    assert!(paths.user_skills_root.join("gamma/SKILL.md").exists());
}
