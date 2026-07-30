use crate::*;
use fs2::FileExt;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::sync::{Mutex, OnceLock};

const USER_SKILLS_MUTATION_LOCK: &str = ".user-skills-mutation.lock";
static ACTIVE_USER_SKILLS_MUTATIONS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

pub(crate) struct UserSkillsMutationLock {
    file: File,
    root: PathBuf,
}

impl Drop for UserSkillsMutationLock {
    fn drop(&mut self) {
        if let Ok(mut active) = active_user_skills_mutations().lock() {
            active.remove(&self.root);
        }
        let _ = FileExt::unlock(&self.file);
    }
}

pub(crate) fn acquire_user_skills_mutation_lock(
    managed_root: &Path,
) -> Result<UserSkillsMutationLock> {
    fs::create_dir_all(managed_root).map_err(|error| error.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(managed_root.join(USER_SKILLS_MUTATION_LOCK))
        .map_err(|error| format!("Unable to open user-skills mutation lock: {error}"))?;
    lock.try_lock_exclusive().map_err(|_| {
        "Another user-skills mutation is already running. Wait for it to finish and retry."
            .to_string()
    })?;
    let root = managed_root.to_path_buf();
    active_user_skills_mutations()
        .lock()
        .map_err(|_| "User-skills mutation lock state is unavailable.".to_string())?
        .insert(root.clone());
    Ok(UserSkillsMutationLock { file: lock, root })
}

pub(crate) fn user_skills_mutation_active(managed_root: &Path) -> bool {
    if active_user_skills_mutations()
        .lock()
        .map(|active| active.contains(managed_root))
        .unwrap_or(true)
    {
        return true;
    }
    let lock_path = managed_root.join(USER_SKILLS_MUTATION_LOCK);
    let Ok(lock) = OpenOptions::new().read(true).write(true).open(lock_path) else {
        return false;
    };
    match lock.try_lock_shared() {
        Ok(()) => {
            let _ = FileExt::unlock(&lock);
            false
        }
        Err(_) => true,
    }
}

fn active_user_skills_mutations() -> &'static Mutex<HashSet<PathBuf>> {
    ACTIVE_USER_SKILLS_MUTATIONS.get_or_init(|| Mutex::new(HashSet::new()))
}
