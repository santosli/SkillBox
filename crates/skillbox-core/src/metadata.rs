use crate::*;

const MAX_SKILL_TAGS: usize = 32;
const MAX_SKILL_TAG_CHARS: usize = 64;

pub fn list_skill_user_metadata(managed_root: impl AsRef<Path>) -> Result<Vec<SkillUserMetadata>> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    load_skill_user_metadata(&paths.database_path)
}

pub fn set_skill_user_metadata(
    request: SkillUserMetadataUpdate,
    managed_root: impl AsRef<Path>,
) -> Result<SkillUserMetadata> {
    validate_skill_name(&request.skill_name)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let metadata = normalized_skill_user_metadata(request);
    upsert_skill_user_metadata(&paths.database_path, &metadata)?;
    Ok(metadata)
}

pub fn migrate_legacy_skill_user_metadata(
    items: Vec<SkillUserMetadataUpdate>,
    managed_root: impl AsRef<Path>,
) -> Result<Vec<SkillUserMetadata>> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut connection = open_database(&paths.database_path)?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;

    for item in items {
        validate_skill_name(&item.skill_name)?;
        let metadata = normalized_skill_user_metadata(item);
        let tags_json = serde_json::to_string(&metadata.tags).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "
                INSERT OR IGNORE INTO skill_user_metadata (
                  skill_name, favorite, tags_json, updated_at
                )
                VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
                ",
                params![
                    metadata.skill_name,
                    if metadata.favorite { 1 } else { 0 },
                    tags_json
                ],
            )
            .map_err(|error| error.to_string())?;
    }

    transaction.commit().map_err(|error| error.to_string())?;
    load_skill_user_metadata(&paths.database_path)
}

fn normalized_skill_user_metadata(request: SkillUserMetadataUpdate) -> SkillUserMetadata {
    let mut tags = Vec::new();
    for tag in request.tags {
        let tag = normalize_skill_tag(&tag);
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
        if tags.len() == MAX_SKILL_TAGS {
            break;
        }
    }

    SkillUserMetadata {
        skill_name: request.skill_name,
        favorite: request.favorite,
        tags,
    }
}

fn normalize_skill_tag(tag: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;
    for character in tag.trim().to_lowercase().chars() {
        let character = if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            character
        } else if character.is_whitespace() {
            '-'
        } else {
            continue;
        };
        if character == '-' && previous_was_separator {
            continue;
        }
        normalized.push(character);
        previous_was_separator = character == '-';
        if normalized.chars().count() == MAX_SKILL_TAG_CHARS {
            break;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn upsert_skill_user_metadata(database_path: &Path, metadata: &SkillUserMetadata) -> Result<()> {
    let connection = open_database(database_path)?;
    let tags_json = serde_json::to_string(&metadata.tags).map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO skill_user_metadata (skill_name, favorite, tags_json, updated_at)
            VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
            ON CONFLICT(skill_name) DO UPDATE SET
              favorite = excluded.favorite,
              tags_json = excluded.tags_json,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                metadata.skill_name,
                if metadata.favorite { 1 } else { 0 },
                tags_json
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn load_skill_user_metadata(database_path: &Path) -> Result<Vec<SkillUserMetadata>> {
    let connection = open_database(database_path)?;
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, favorite, tags_json
            FROM skill_user_metadata
            ORDER BY skill_name
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? != 0,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut metadata = Vec::new();
    for row in rows {
        let (skill_name, favorite, tags_json) = row.map_err(|error| error.to_string())?;
        let tags = serde_json::from_str::<Vec<String>>(&tags_json)
            .map_err(|error| format!("Invalid tags for {skill_name}: {error}"))?;
        metadata.push(SkillUserMetadata {
            skill_name,
            favorite,
            tags,
        });
    }
    Ok(metadata)
}
