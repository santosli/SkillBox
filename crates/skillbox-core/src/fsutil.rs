use crate::*;

pub(crate) fn find_skill_dirs(
    current: &Path,
    depth: usize,
    max_depth: usize,
    found: &mut Vec<PathBuf>,
) -> Result<()> {
    if depth > max_depth {
        return Ok(());
    }
    let current_metadata = fs::symlink_metadata(current).map_err(|error| error.to_string())?;
    if current_metadata.file_type().is_symlink() {
        return Ok(());
    }
    if current.join("SKILL.md").exists() {
        found.push(current.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if !file_type.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with('.') && file_name != ".system" {
            continue;
        }
        find_skill_dirs(&path, depth + 1, max_depth, found)?;
    }

    Ok(())
}

pub(crate) fn resolve_managed_skill_path(
    paths: &ManagedPaths,
    skill_name: &str,
) -> Result<PathBuf> {
    let user_path = paths.user_skills_root.join(skill_name);
    if user_path.join("SKILL.md").exists() {
        return Ok(user_path);
    }

    let remote_current = paths.remote_skills_root.join(skill_name).join("current");
    if remote_current.join("SKILL.md").exists() {
        return Ok(remote_current);
    }

    Err(format!("Managed skill not found: {skill_name}"))
}

pub(crate) fn copy_skill_dir(source: &Path, destination: &Path) -> Result<()> {
    copy_skill_dir_with_link_root(source, destination, None)
}

pub(crate) fn copy_skill_dir_from_checkout(
    source: &Path,
    destination: &Path,
    checkout_root: &Path,
) -> Result<()> {
    copy_skill_dir_with_link_root(source, destination, Some(checkout_root))
}

fn copy_skill_dir_with_link_root(
    source: &Path,
    destination: &Path,
    link_root: Option<&Path>,
) -> Result<()> {
    if destination.exists() {
        return Err(format!(
            "Destination already exists: {}",
            destination.display()
        ));
    }
    let temp_destination = temporary_sibling_path(destination, "copy")?;
    if temp_destination.exists() {
        return Err(format!(
            "Temporary destination already exists: {}",
            temp_destination.display()
        ));
    }
    let source_root = fs::canonicalize(source).map_err(|error| error.to_string())?;
    let link_root = link_root
        .map(fs::canonicalize)
        .transpose()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&temp_destination).map_err(|error| error.to_string())?;

    let result = (|| {
        for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_name = entry.file_name();
            if file_name == ".git" {
                continue;
            }
            copy_recursively(
                &entry.path(),
                &temp_destination.join(file_name),
                &source_root,
                link_root.as_deref(),
            )?;
        }
        fs::rename(&temp_destination, destination).map_err(|error| error.to_string())
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&temp_destination);
    }
    result
}

pub(crate) fn copy_recursively(
    source: &Path,
    destination: &Path,
    source_root: &Path,
    link_root: Option<&Path>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|error| error.to_string())?;
        for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            copy_recursively(
                &entry.path(),
                &destination.join(entry.file_name()),
                source_root,
                link_root,
            )?;
        }
    } else if metadata.file_type().is_symlink() {
        let target = fs::read_link(source).map_err(|error| error.to_string())?;
        let checked_target = symlink_target_for_boundary_check(source, &target)?;
        if checked_target.starts_with(source_root) {
            symlink_any(&target, destination)?;
        } else if link_root
            .map(|root| checked_target.starts_with(root))
            .unwrap_or(false)
        {
            if !checked_target.exists() {
                return Err(format!(
                    "Symlink target does not exist inside checkout: {}",
                    checked_target.display()
                ));
            }
            copy_recursively(&checked_target, destination, source_root, None)?;
        } else {
            return Err(format!(
                "Refusing to copy symlink outside source root: {}",
                source.display()
            ));
        }
    } else {
        fs::copy(source, destination).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn symlink_target_for_boundary_check(source: &Path, target: &Path) -> Result<PathBuf> {
    let absolute_target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        source
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };

    match fs::canonicalize(&absolute_target) {
        Ok(target) => Ok(target),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let lexical_target = if target.is_absolute() {
                absolute_target
            } else {
                let base = source
                    .parent()
                    .and_then(|parent| fs::canonicalize(parent).ok())
                    .unwrap_or_else(|| {
                        source
                            .parent()
                            .unwrap_or_else(|| Path::new(""))
                            .to_path_buf()
                    });
                base.join(target)
            };
            Ok(normalize_lexical_path(&lexical_target))
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn temporary_sibling_path(destination: &Path, label: &str) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| format!("Destination has no parent: {}", destination.display()))?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skill");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    Ok(parent.join(format!(".{name}.{label}-{nanos}.tmp")))
}

pub(crate) fn update_current_symlink(remote_root: &Path, version_path: &Path) -> Result<()> {
    fs::create_dir_all(remote_root).map_err(|error| error.to_string())?;
    let current = remote_root.join("current");
    match fs::symlink_metadata(&current) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(format!(
                    "Refusing to replace existing non-symlink current: {}",
                    current.display()
                ));
            }
            fs::remove_file(&current).map_err(|error| error.to_string())?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    symlink_dir(version_path, &current)
}

pub(crate) fn symlink_points_to_path(symlink: &Path, expected: &Path) -> Result<bool> {
    let target = fs::read_link(symlink).map_err(|error| error.to_string())?;
    let target = if target.is_absolute() {
        target
    } else {
        symlink
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    Ok(target == expected)
}

pub(crate) fn validate_skill_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(format!("Invalid skill name: {name}"));
    }
    Ok(())
}

pub(crate) fn expand_home(path: PathBuf) -> PathBuf {
    let path_string = path.to_string_lossy();
    if path_string == "~" {
        return home_dir();
    }
    if let Some(rest) = path_string.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    path
}

pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub(crate) fn unquote(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn sha256(content: &str) -> String {
    sha256_bytes(content.as_bytes())
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to a String should not fail");
    }
    output
}

#[cfg(unix)]
pub(crate) fn symlink_dir(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, destination).map_err(|error| error.to_string())
}

#[cfg(unix)]
pub(crate) fn symlink_any(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, destination).map_err(|error| error.to_string())
}
