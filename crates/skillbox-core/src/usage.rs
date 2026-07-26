use crate::*;

type UsageRankingRootAggregates = HashMap<(String, String, SkillUsageSourceKind), UsageSummary>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UsageEvidenceSource {
    source: String,
    evidence_class: SkillUsageEvidenceClass,
}

pub fn record_skill_usage(
    request: RecordSkillUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<SkillUsageRecordResult> {
    reject_reserved_usage_source(&request)?;
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    record_skill_usage_on_connection(request, &mut connection, false)
}

fn reject_reserved_usage_source(request: &RecordSkillUsageRequest) -> Result<()> {
    let source = request
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("source"))
        .and_then(|value| value.as_str());
    if matches!(
        source,
        Some(
            "agent_hook"
                | "codex_session_backfill"
                | "claude_code_session_backfill"
                | "cursor_session_backfill"
                | "cursor_agent_transcript_read"
        )
    ) {
        return Err(
            "metadata.source is reserved for SkillBox trusted hook and backfill events."
                .to_string(),
        );
    }
    Ok(())
}

pub(crate) fn record_trusted_generated_skill_usage(
    request: RecordSkillUsageRequest,
    managed_root: impl AsRef<Path>,
) -> Result<SkillUsageRecordResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let mut connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    record_skill_usage_on_connection(request, &mut connection, true)
}

pub(crate) fn record_skill_usage_on_connection(
    request: RecordSkillUsageRequest,
    connection: &mut Connection,
    trusted_generated_source: bool,
) -> Result<SkillUsageRecordResult> {
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let result =
        record_skill_usage_in_transaction(request, &transaction, trusted_generated_source)?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(result)
}

fn record_skill_usage_in_transaction(
    request: RecordSkillUsageRequest,
    connection: &Connection,
    trusted_generated_source: bool,
) -> Result<SkillUsageRecordResult> {
    let skill_name = request.skill_name.trim().to_string();
    validate_skill_name(&skill_name)?;
    let agent_id = normalize_usage_agent_id(&request.agent_id)?;
    let mut runtime_root = normalize_usage_runtime_root(request.runtime_root)?;
    let mut runtime_root_value = runtime_root.to_string_lossy().to_string();
    let event_id = normalize_usage_event_id(request.event_id)?;
    let used_at = normalize_usage_timestamp(request.used_at.as_deref())?;
    let recorded_at = current_rfc3339_timestamp();
    let prompt_excerpt = normalize_usage_prompt_excerpt(request.prompt_excerpt.as_deref());
    let metadata_json = normalize_usage_metadata(request.metadata)?;
    let (incoming_evidence_class, incoming_evidence_source) =
        classify_usage_evidence(&metadata_json, trusted_generated_source);
    if let Some(existing_runtime_root) = generated_usage_event_runtime(
        connection,
        &skill_name,
        &agent_id,
        event_id.as_deref(),
        &metadata_json,
        trusted_generated_source,
    )? {
        runtime_root = PathBuf::from(&existing_runtime_root);
        runtime_root_value = existing_runtime_root;
    }
    canonicalize_runtime_usage_agent_aliases(connection, &agent_id, &runtime_root_value)?;

    if let Some(event_id_value) = event_id.as_deref() {
        if let Some(existing) =
            find_existing_usage_event(connection, &agent_id, &runtime_root_value, event_id_value)?
        {
            let (
                existing_agent_id,
                existing_used_at,
                existing_recorded_at,
                existing_prompt_excerpt,
                existing_metadata_json,
                existing_evidence_class,
                existing_evidence_sources_json,
            ) = existing;
            if existing_agent_id != agent_id {
                connection
                    .execute(
                        "
                        UPDATE skill_usage_events
                        SET agent_id = ?1
                        WHERE agent_id = ?2
                          AND runtime_root = ?3
                          AND event_id = ?4
                        ",
                        params![
                            &agent_id,
                            &existing_agent_id,
                            &runtime_root_value,
                            event_id_value,
                        ],
                    )
                    .map_err(|error| error.to_string())?;
                merge_legacy_usage_stat_into_canonical(
                    connection,
                    &skill_name,
                    &existing_agent_id,
                    &agent_id,
                    &runtime_root_value,
                )?;
            }
            if existing_prompt_excerpt.is_none() {
                if let Some(prompt_excerpt_value) = prompt_excerpt.as_deref() {
                    connection
                        .execute(
                            "
                            UPDATE skill_usage_events
                            SET prompt_excerpt = ?1
                            WHERE agent_id = ?2
                              AND runtime_root = ?3
                              AND event_id = ?4
                              AND prompt_excerpt IS NULL
                            ",
                            params![
                                prompt_excerpt_value,
                                &agent_id,
                                &runtime_root_value,
                                event_id_value,
                            ],
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
            if let Some(merged_metadata_json) =
                merge_usage_source_identity(&existing_metadata_json, &metadata_json)?
            {
                connection
                    .execute(
                        "
                        UPDATE skill_usage_events
                        SET metadata_json = ?1
                        WHERE agent_id = ?2
                          AND runtime_root = ?3
                          AND event_id = ?4
                        ",
                        params![
                            merged_metadata_json,
                            &agent_id,
                            &runtime_root_value,
                            event_id_value,
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            let merged_evidence_sources_json = merge_usage_evidence_sources(
                &existing_evidence_sources_json,
                &incoming_evidence_source,
                incoming_evidence_class,
            )?;
            if merged_evidence_sources_json != existing_evidence_sources_json {
                connection
                    .execute(
                        "
                        UPDATE skill_usage_events
                        SET evidence_sources_json = ?1
                        WHERE agent_id = ?2
                          AND runtime_root = ?3
                          AND event_id = ?4
                        ",
                        params![
                            merged_evidence_sources_json,
                            &agent_id,
                            &runtime_root_value,
                            event_id_value,
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            let existing_evidence_class = parse_usage_evidence_class(&existing_evidence_class)?;
            let upgraded = usage_evidence_rank(incoming_evidence_class)
                > usage_evidence_rank(existing_evidence_class);
            let evidence_class = if upgraded {
                connection
                    .execute(
                        "
                        UPDATE skill_usage_events
                        SET evidence_class = ?1
                        WHERE agent_id = ?2
                          AND runtime_root = ?3
                          AND event_id = ?4
                        ",
                        params![
                            incoming_evidence_class.as_str(),
                            &agent_id,
                            &runtime_root_value,
                            event_id_value
                        ],
                    )
                    .map_err(|error| error.to_string())?;
                if !usage_evidence_counts_toward_calls(existing_evidence_class)
                    && usage_evidence_counts_toward_calls(incoming_evidence_class)
                {
                    increment_call_usage_stat(
                        connection,
                        &skill_name,
                        &agent_id,
                        &runtime_root_value,
                        &existing_used_at,
                    )?;
                }
                incoming_evidence_class
            } else {
                existing_evidence_class
            };
            let usage =
                load_usage_stat_for_key(connection, &skill_name, &agent_id, &runtime_root_value)?;
            return Ok(SkillUsageRecordResult {
                skill_name,
                agent_id,
                runtime_root,
                event_id,
                used_at: existing_used_at.clone(),
                recorded_at: existing_recorded_at,
                usage_count: usage.usage_count,
                last_used_at: usage.last_used_at.unwrap_or(existing_used_at),
                deduplicated: true,
                evidence_class,
                upgraded,
            });
        }
    }

    connection
        .execute(
            "
            INSERT INTO skill_usage_events (
              id,
              event_id,
              skill_name,
              agent_id,
              runtime_root,
              used_at,
              recorded_at,
              prompt_excerpt,
              metadata_json,
              evidence_class,
              evidence_sources_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                usage_event_row_id(),
                event_id.as_deref(),
                &skill_name,
                &agent_id,
                &runtime_root_value,
                &used_at,
                &recorded_at,
                prompt_excerpt.as_deref(),
                &metadata_json,
                incoming_evidence_class.as_str(),
                serde_json::to_string(&vec![UsageEvidenceSource {
                    source: incoming_evidence_source,
                    evidence_class: incoming_evidence_class,
                }])
                .map_err(|error| error.to_string())?,
            ],
        )
        .map_err(|error| error.to_string())?;
    if usage_evidence_counts_toward_calls(incoming_evidence_class) {
        increment_call_usage_stat(
            connection,
            &skill_name,
            &agent_id,
            &runtime_root_value,
            &used_at,
        )?;
    }

    let usage = load_usage_stat_for_key(connection, &skill_name, &agent_id, &runtime_root_value)?;
    Ok(SkillUsageRecordResult {
        skill_name,
        agent_id,
        runtime_root,
        event_id,
        used_at,
        recorded_at,
        usage_count: usage.usage_count,
        last_used_at: usage.last_used_at.unwrap_or_default(),
        deduplicated: false,
        evidence_class: incoming_evidence_class,
        upgraded: false,
    })
}

fn increment_call_usage_stat(
    connection: &Connection,
    skill_name: &str,
    agent_id: &str,
    runtime_root: &str,
    used_at: &str,
) -> Result<()> {
    connection
        .execute(
            "
            INSERT INTO skill_usage_stats (
              skill_name,
              agent_id,
              runtime_root,
              usage_count,
              last_used_at
            )
            VALUES (?1, ?2, ?3, 1, ?4)
            ON CONFLICT(skill_name, agent_id, runtime_root) DO UPDATE SET
              usage_count = skill_usage_stats.usage_count + 1,
              last_used_at = CASE
                WHEN excluded.last_used_at > skill_usage_stats.last_used_at
                THEN excluded.last_used_at
                ELSE skill_usage_stats.last_used_at
              END,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![skill_name, agent_id, runtime_root, used_at],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn list_skill_usage_rankings(
    request: SkillUsageRankingRequest,
    managed_root: impl AsRef<Path>,
) -> Result<SkillUsageRankingResult> {
    list_skill_usage_rankings_at(request, managed_root, Utc::now())
}

pub fn preview_usage_skill_import(
    skill_name: impl AsRef<str>,
    managed_root: impl AsRef<Path>,
) -> Result<ImportCandidate> {
    preview_usage_skill_import_impl(
        PreviewUsageSkillImportRequest {
            skill_name: skill_name.as_ref().to_string(),
            source_kind: Some(SkillUsageSourceKind::Regular),
            source_id: None,
            source_runtime_roots: Vec::new(),
            ranking_request: None,
            ranking_generated_at: None,
            runtime_root: None,
        },
        managed_root,
        false,
    )
}

pub fn preview_usage_skill_import_for_source(
    request: PreviewUsageSkillImportRequest,
    managed_root: impl AsRef<Path>,
) -> Result<ImportCandidate> {
    preview_usage_skill_import_impl(request, managed_root, true)
}

fn preview_usage_skill_import_impl(
    request: PreviewUsageSkillImportRequest,
    managed_root: impl AsRef<Path>,
    require_source_identity: bool,
) -> Result<ImportCandidate> {
    let skill_name = request.skill_name.trim();
    validate_skill_name(skill_name)?;
    let source_kind = request.source_kind.unwrap_or(SkillUsageSourceKind::Regular);
    if source_kind == SkillUsageSourceKind::System {
        return Err(format!(
            "System skill `{skill_name}` cannot be imported into SkillBox."
        ));
    }
    if source_kind == SkillUsageSourceKind::Unknown {
        return Err(format!(
            "Skill `{skill_name}` has an unknown historical source and cannot be imported from Rankings."
        ));
    }
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    if resolve_managed_skill_path(&paths, skill_name).is_ok() {
        return Err(format!(
            "Skill `{skill_name}` is already imported into SkillBox."
        ));
    }

    let mut preferred_roots = request
        .source_runtime_roots
        .into_iter()
        .map(normalize_usage_runtime_root)
        .collect::<Result<Vec<_>>>()?;
    if let Some(runtime_root) = request.runtime_root {
        preferred_roots.push(normalize_usage_runtime_root(runtime_root)?);
    }
    preferred_roots = sorted_usage_roots(preferred_roots);
    if require_source_identity {
        let source_id = request.source_id.as_deref().ok_or_else(|| {
            "Ranking source identity is required. Refresh Rankings and try again.".to_string()
        })?;
        let ranking_request = request.ranking_request.ok_or_else(|| {
            "Ranking query identity is required. Refresh Rankings and try again.".to_string()
        })?;
        let ranking_generated_at = request.ranking_generated_at.as_deref().ok_or_else(|| {
            "Ranking snapshot time is required. Refresh Rankings and try again.".to_string()
        })?;
        let ranking_as_of = DateTime::parse_from_rfc3339(ranking_generated_at)
            .map_err(|_| "Ranking snapshot time is invalid. Refresh Rankings and try again.")?
            .with_timezone(&Utc);
        let ranking = list_skill_usage_rankings_at(ranking_request, &paths.root, ranking_as_of)?;
        let source_row = ranking
            .rows
            .into_iter()
            .find(|row| {
                row.source_id == source_id
                    && row.skill_name == skill_name
                    && row.source_kind == source_kind
                    && !row.managed
            })
            .ok_or_else(|| {
                format!("Ranking source `{source_id}` is stale. Refresh Rankings and try again.")
            })?;
        let source_roots = sorted_usage_roots(source_row.source_runtime_roots);
        if source_roots.is_empty()
            || preferred_roots
                .iter()
                .map(|root| usage_runtime_key(root))
                .ne(source_roots.iter().map(|root| usage_runtime_key(root)))
        {
            return Err(format!(
                "Ranking source `{source_id}` no longer matches the displayed row. Refresh Rankings and try again."
            ));
        }
        preferred_roots = source_roots;
    }
    let roots = usage_skill_import_roots(
        &paths,
        skill_name,
        &preferred_roots,
        !require_source_identity,
    )?;
    if roots.is_empty() {
        return Err(format!(
            "No recoverable local source found for skill `{skill_name}`. Reinstall it from a runtime folder or GitHub."
        ));
    }

    let scan = scan_import_candidates(&roots, &paths.root)?;
    let source_matches = |candidate: &ImportCandidate| {
        preferred_roots.is_empty()
            || preferred_roots.iter().any(|preferred| {
                candidate
                    .source_root
                    .as_ref()
                    .is_some_and(|root| usage_runtime_key(root) == usage_runtime_key(preferred))
                    || candidate.source_path.starts_with(preferred)
            })
    };
    let candidate = scan
        .candidates
        .iter()
        .filter(|candidate| candidate.name == skill_name)
        .filter(|candidate| candidate.import_status == ImportCandidateStatus::Importable)
        .find(|candidate| source_matches(candidate))
        .cloned();
    let Some(candidate) = candidate else {
        if scan.candidates.iter().any(|candidate| {
            candidate.name == skill_name
                && candidate.import_status == ImportCandidateStatus::System
                && source_matches(candidate)
        }) {
            return Err(format!(
                "Skill `{skill_name}` is not importable from a System source."
            ));
        }
        return Err(format!(
            "Unable to locate skill `{skill_name}` under recoverable local sources."
        ));
    };

    if let Some(conflict) = candidate.conflict.as_deref() {
        return Err(format!(
            "Skill `{skill_name}` cannot be imported: {conflict}"
        ));
    }

    Ok(ImportCandidate {
        is_selected: true,
        ..candidate
    })
}

fn usage_skill_import_roots(
    paths: &ManagedPaths,
    skill_name: &str,
    preferred_roots: &[PathBuf],
    include_fallbacks: bool,
) -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    for root in preferred_roots {
        push_usage_import_root(&mut roots, root.clone(), paths);
    }
    if include_fallbacks {
        for root in recorded_usage_runtime_roots(&paths.database_path, skill_name)? {
            push_usage_import_root(&mut roots, root, paths);
        }
        for root in global_runtime_roots() {
            push_usage_import_root(&mut roots, root, paths);
        }
        if let Some(backup_root) = latest_deletion_backup_import_root(paths, skill_name) {
            push_usage_import_root(&mut roots, backup_root, paths);
        }
    }
    Ok(roots)
}

fn recorded_usage_runtime_roots(database_path: &Path, skill_name: &str) -> Result<Vec<PathBuf>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT runtime_root
            FROM skill_usage_events
            WHERE skill_name = ?1
            ORDER BY used_at DESC, recorded_at DESC
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![skill_name], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let runtime_root = row.map_err(|error| error.to_string())?;
        if !seen.insert(runtime_root.clone()) {
            continue;
        }
        roots.push(PathBuf::from(runtime_root));
    }
    Ok(roots)
}

fn push_usage_import_root(roots: &mut Vec<PathBuf>, root: PathBuf, paths: &ManagedPaths) {
    if !root.is_dir() {
        return;
    }
    if is_under_path(&root, &paths.root) && !is_under_path(&root, &paths.root.join("backups")) {
        return;
    }
    let key = usage_runtime_key(&root);
    if roots
        .iter()
        .any(|existing| usage_runtime_key(existing) == key)
    {
        return;
    }
    roots.push(root);
}

fn latest_deletion_backup_import_root(paths: &ManagedPaths, skill_name: &str) -> Option<PathBuf> {
    let deletions_root = paths.root.join("backups").join("deletions");
    let entries = fs::read_dir(&deletions_root).ok()?;
    let prefix = format!("{skill_name}-");
    let mut backups = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix))
        })
        .collect::<Vec<_>>();
    backups.sort_by(|left, right| right.cmp(left));
    for backup in backups {
        if backup.join("SKILL.md").is_file() {
            return Some(backup);
        }
        let current = backup.join("current");
        if current.join("SKILL.md").is_file() {
            return Some(fs::canonicalize(&current).unwrap_or(current));
        }
        let versions = backup.join("versions");
        if let Some(version) = newest_skill_version_dir(&versions) {
            return Some(version);
        }
    }
    None
}

fn newest_skill_version_dir(versions_root: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(versions_root).ok()?;
    let mut versions = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("SKILL.md").is_file())
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| {
        let left_modified = fs::metadata(left)
            .and_then(|metadata| metadata.modified())
            .ok();
        let right_modified = fs::metadata(right)
            .and_then(|metadata| metadata.modified())
            .ok();
        right_modified
            .cmp(&left_modified)
            .then_with(|| right.cmp(left))
    });
    versions.into_iter().next()
}

pub(crate) fn list_skill_usage_rankings_at(
    request: SkillUsageRankingRequest,
    managed_root: impl AsRef<Path>,
    as_of: DateTime<Utc>,
) -> Result<SkillUsageRankingResult> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let agent_id = request
        .agent_id
        .as_deref()
        .map(normalize_usage_agent_id)
        .transpose()?;
    let agent_filter_ids = agent_id
        .as_deref()
        .map(usage_ranking_agent_filter_ids)
        .unwrap_or_default();
    let workspace_root = request
        .workspace_root
        .map(normalize_usage_runtime_root)
        .transpose()?;
    let workspace_key = workspace_root
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let range_end = as_of.to_rfc3339_opts(SecondsFormat::Secs, false);
    let range_start = usage_ranking_range_start(request.range, as_of);
    let query_start = range_start
        .as_deref()
        .unwrap_or("0001-01-01T00:00:00+00:00");
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let managed = load_managed_skill_kinds(&paths)?;
    let (root_aggregates, mut coverage) = load_usage_ranking_root_aggregates(
        &connection,
        RankingFilterBounds {
            range_start: query_start,
            range_end: &range_end,
            agent_ids: &agent_filter_ids,
            workspace_root: workspace_key.as_deref(),
        },
        &managed,
        request.include_unmanaged,
        request.skill_type,
    )?;
    coverage.scanned_codex_session_files =
        read_u32_preference(&paths.database_path, "codex_usage_backfill_scanned_files")?
            .unwrap_or_default() as usize;
    coverage.scanned_codex_turns =
        read_u32_preference(&paths.database_path, "codex_usage_backfill_scanned_turns")?
            .unwrap_or_default() as usize;
    coverage.scanned_claude_code_session_files = read_u32_preference(
        &paths.database_path,
        "claude_code_usage_backfill_scanned_files",
    )?
    .unwrap_or_default() as usize;
    coverage.scanned_cursor_sessions = read_u32_preference(
        &paths.database_path,
        "cursor_usage_backfill_scanned_sessions",
    )?
    .unwrap_or_default() as usize;
    coverage.scanned_cursor_transcript_files = read_u32_preference(
        &paths.database_path,
        "cursor_usage_backfill_scanned_transcript_files",
    )?
    .unwrap_or_default() as usize;
    let mut system_aggregates: HashMap<String, UsageSummary> = HashMap::new();
    let mut regular_aggregates: HashMap<String, UsageSummary> = HashMap::new();
    let mut unknown_aggregates: HashMap<String, UsageSummary> = HashMap::new();
    let mut system_roots: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut regular_roots: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut unknown_roots: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for ((skill_name, runtime_root, source_kind), summary) in root_aggregates {
        let root = PathBuf::from(&runtime_root);
        let (aggregates, roots) = match source_kind {
            SkillUsageSourceKind::Regular => (&mut regular_aggregates, &mut regular_roots),
            SkillUsageSourceKind::System => (&mut system_aggregates, &mut system_roots),
            SkillUsageSourceKind::Unknown => (&mut unknown_aggregates, &mut unknown_roots),
        };
        merge_usage_summary(aggregates.entry(skill_name.clone()).or_default(), &summary);
        let entry = roots.entry(skill_name).or_default();
        if !entry.iter().any(|existing| existing == &root) {
            entry.push(root);
        }
    }

    let mut rows = managed
        .iter()
        .filter(|(skill_name, _)| {
            usage_ranking_matches_skill_type(
                skill_name,
                SkillUsageSourceKind::Regular,
                &managed,
                request.skill_type,
            )
        })
        .map(|(skill_name, kind)| {
            let summary = regular_aggregates.remove(skill_name).unwrap_or_default();
            let roots = regular_roots.remove(skill_name).unwrap_or_default();
            SkillUsageRankingRow {
                rank: 0,
                skill_name: skill_name.clone(),
                kind: Some(*kind),
                managed: true,
                system: false,
                source_missing: false,
                source_kind: SkillUsageSourceKind::Regular,
                source_id: usage_ranking_source_id(
                    skill_name,
                    SkillUsageSourceKind::Regular,
                    &roots,
                ),
                source_runtime_roots: sorted_usage_roots(roots),
                usage_count: summary.usage_count,
                last_used_at: summary.last_used_at,
                confirmed_count: summary.confirmed_count,
                inferred_count: summary.inferred_count,
                reference_count: summary.reference_count,
                last_referenced_at: summary.last_referenced_at,
            }
        })
        .collect::<Vec<_>>();

    if request.include_unmanaged {
        rows.extend(regular_aggregates.into_iter().map(|(skill_name, summary)| {
            let roots = regular_roots.remove(&skill_name).unwrap_or_default();
            let source_missing = !unmanaged_regular_skill_source_present(&skill_name, &roots);
            let source_id =
                usage_ranking_source_id(&skill_name, SkillUsageSourceKind::Regular, &roots);
            SkillUsageRankingRow {
                rank: 0,
                skill_name,
                kind: None,
                managed: false,
                system: false,
                source_missing,
                source_kind: SkillUsageSourceKind::Regular,
                source_id,
                source_runtime_roots: sorted_usage_roots(roots),
                usage_count: summary.usage_count,
                last_used_at: summary.last_used_at,
                confirmed_count: summary.confirmed_count,
                inferred_count: summary.inferred_count,
                reference_count: summary.reference_count,
                last_referenced_at: summary.last_referenced_at,
            }
        }));
        rows.extend(system_aggregates.into_iter().map(|(skill_name, summary)| {
            let roots = system_roots.remove(&skill_name).unwrap_or_default();
            let source_missing = !unmanaged_system_skill_source_present(&skill_name, &roots);
            let source_id =
                usage_ranking_source_id(&skill_name, SkillUsageSourceKind::System, &roots);
            SkillUsageRankingRow {
                rank: 0,
                skill_name,
                kind: None,
                managed: false,
                system: true,
                source_missing,
                source_kind: SkillUsageSourceKind::System,
                source_id,
                source_runtime_roots: sorted_usage_roots(roots),
                usage_count: summary.usage_count,
                last_used_at: summary.last_used_at,
                confirmed_count: summary.confirmed_count,
                inferred_count: summary.inferred_count,
                reference_count: summary.reference_count,
                last_referenced_at: summary.last_referenced_at,
            }
        }));
        rows.extend(unknown_aggregates.into_iter().map(|(skill_name, summary)| {
            let roots = unknown_roots.remove(&skill_name).unwrap_or_default();
            let source_id =
                usage_ranking_source_id(&skill_name, SkillUsageSourceKind::Unknown, &roots);
            SkillUsageRankingRow {
                rank: 0,
                skill_name,
                kind: None,
                managed: false,
                system: false,
                source_missing: false,
                source_kind: SkillUsageSourceKind::Unknown,
                source_id,
                source_runtime_roots: sorted_usage_roots(roots),
                usage_count: summary.usage_count,
                last_used_at: summary.last_used_at,
                confirmed_count: summary.confirmed_count,
                inferred_count: summary.inferred_count,
                reference_count: summary.reference_count,
                last_referenced_at: summary.last_referenced_at,
            }
        }));
    }

    rows.sort_by(|left, right| {
        right
            .usage_count
            .cmp(&left.usage_count)
            .then_with(|| right.last_used_at.cmp(&left.last_used_at))
            .then_with(|| left.skill_name.cmp(&right.skill_name))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    for (index, row) in rows.iter_mut().enumerate() {
        row.rank = index + 1;
    }
    let total_calls = rows.iter().map(|row| row.usage_count).sum();
    let total_confirmed_calls = rows.iter().map(|row| row.confirmed_count).sum();
    let total_inferred_calls = rows.iter().map(|row| row.inferred_count).sum();
    let total_history_references = rows.iter().map(|row| row.reference_count).sum();

    Ok(SkillUsageRankingResult {
        generated_at: range_end.clone(),
        range: request.range,
        range_start,
        range_end,
        agent_id,
        skill_type: request.skill_type,
        workspace_root,
        total_calls,
        total_observed_calls: total_calls,
        total_confirmed_calls,
        total_inferred_calls,
        total_history_references,
        coverage,
        rows,
    })
}

pub fn usage_audit(managed_root: impl AsRef<Path>) -> Result<SkillUsageAudit> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let connection = open_database(&paths.database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT evidence_class, evidence_sources_json, used_at
            FROM skill_usage_events
            ORDER BY used_at, id
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut audit = SkillUsageAudit::default();
    let mut source_counts: HashMap<(String, SkillUsageEvidenceClass), usize> = HashMap::new();
    for row in rows {
        let (evidence_class, evidence_sources_json, used_at) =
            row.map_err(|error| error.to_string())?;
        let evidence_class = parse_usage_evidence_class(&evidence_class)?;
        match evidence_class {
            SkillUsageEvidenceClass::Confirmed => {
                audit.confirmed_calls = audit.confirmed_calls.saturating_add(1);
                update_coverage_timestamp(
                    &mut audit.earliest_confirmed_at,
                    &mut audit.latest_confirmed_at,
                    &used_at,
                );
            }
            SkillUsageEvidenceClass::Inferred => {
                audit.inferred_calls = audit.inferred_calls.saturating_add(1);
                update_coverage_timestamp(
                    &mut audit.earliest_inferred_at,
                    &mut audit.latest_inferred_at,
                    &used_at,
                );
            }
            SkillUsageEvidenceClass::Reference => {
                audit.history_references = audit.history_references.saturating_add(1);
                update_coverage_timestamp(
                    &mut audit.earliest_reference_at,
                    &mut audit.latest_reference_at,
                    &used_at,
                );
            }
        }
        for source in parse_usage_evidence_sources(&evidence_sources_json).unwrap_or_default() {
            *source_counts
                .entry((
                    usage_evidence_source_bucket(&source.source).to_string(),
                    source.evidence_class,
                ))
                .or_default() += 1;
        }
    }
    audit.source_counts = source_counts
        .into_iter()
        .map(
            |((source, evidence_class), count)| SkillUsageEvidenceSourceCount {
                source,
                evidence_class,
                count,
            },
        )
        .collect();
    audit.source_counts.sort_by(|left, right| {
        left.source.cmp(&right.source).then_with(|| {
            left.evidence_class
                .as_str()
                .cmp(right.evidence_class.as_str())
        })
    });
    audit.scanned_codex_session_files =
        read_u32_preference(&paths.database_path, "codex_usage_backfill_scanned_files")?
            .unwrap_or_default() as usize;
    audit.scanned_codex_turns =
        read_u32_preference(&paths.database_path, "codex_usage_backfill_scanned_turns")?
            .unwrap_or_default() as usize;
    audit.scanned_claude_code_session_files = read_u32_preference(
        &paths.database_path,
        "claude_code_usage_backfill_scanned_files",
    )?
    .unwrap_or_default() as usize;
    audit.scanned_cursor_sessions = read_u32_preference(
        &paths.database_path,
        "cursor_usage_backfill_scanned_sessions",
    )?
    .unwrap_or_default() as usize;
    audit.scanned_cursor_transcript_files = read_u32_preference(
        &paths.database_path,
        "cursor_usage_backfill_scanned_transcript_files",
    )?
    .unwrap_or_default() as usize;
    audit.confirmed_cursor_transcript_reads = audit
        .source_counts
        .iter()
        .find(|item| item.source == "cursor_agent_transcript_read")
        .map(|item| item.count)
        .unwrap_or_default();
    audit.total_calls = audit.confirmed_calls.saturating_add(audit.inferred_calls);
    audit.known_limitations.push(
        "Codex local stores do not expose a stable provider-reported skill-run total. Calls include confirmed hooks and structured per-turn inferred invocations, but may still undercount Codex usage."
            .to_string(),
    );
    for source in [
        "codex_session_backfill",
        "claude_code_session_backfill",
        "cursor_session_backfill",
        "cursor_agent_transcript_read",
    ] {
        if let Some(backfill) = read_json_preference::<UsageBackfillAudit>(
            &paths.database_path,
            &format!("usage_backfill_audit_{source}"),
        )? {
            audit.last_backfills.push(backfill);
        }
    }
    Ok(audit)
}

pub(crate) fn persist_usage_backfill_audit(
    database_path: &Path,
    source: &str,
    scanned: usize,
    result: &BackfillCodexSessionUsageResult,
) -> Result<()> {
    write_json_preference(
        database_path,
        &format!("usage_backfill_audit_{source}"),
        &UsageBackfillAudit {
            source: source.to_string(),
            scanned,
            discovered: result.discovered,
            recorded: result.recorded,
            deduplicated: result.deduplicated,
            upgraded: result.upgraded,
            skipped: result.skipped,
            errors: result.errors.len(),
        },
    )
}

fn usage_ranking_range_start(
    range: SkillUsageRankingRange,
    as_of: DateTime<Utc>,
) -> Option<String> {
    let days = match range {
        SkillUsageRankingRange::Last7Days => 7,
        SkillUsageRankingRange::Last30Days => 30,
        SkillUsageRankingRange::AllTime => return None,
    };
    Some((as_of - chrono::Duration::days(days)).to_rfc3339_opts(SecondsFormat::Secs, false))
}

fn load_managed_skill_kinds(paths: &ManagedPaths) -> Result<HashMap<String, SkillKind>> {
    let mut managed = HashMap::new();
    for skill in scan_skill_roots(std::slice::from_ref(&paths.user_skills_root))?.skills {
        managed.insert(skill.name, SkillKind::User);
    }
    for skill in scan_managed_remote_skills(paths)? {
        managed.insert(skill.name, SkillKind::Remote);
    }
    Ok(managed)
}

fn merge_usage_summary(target: &mut UsageSummary, other: &UsageSummary) {
    target.usage_count = target.usage_count.saturating_add(other.usage_count);
    target.confirmed_count = target.confirmed_count.saturating_add(other.confirmed_count);
    target.inferred_count = target.inferred_count.saturating_add(other.inferred_count);
    target.reference_count = target.reference_count.saturating_add(other.reference_count);
    match (&target.last_used_at, &other.last_used_at) {
        (Some(left), Some(right)) if right > left => {
            target.last_used_at = Some(right.clone());
        }
        (None, Some(right)) => {
            target.last_used_at = Some(right.clone());
        }
        _ => {}
    }
    match (&target.last_referenced_at, &other.last_referenced_at) {
        (Some(left), Some(right)) if right > left => {
            target.last_referenced_at = Some(right.clone());
        }
        (None, Some(right)) => {
            target.last_referenced_at = Some(right.clone());
        }
        _ => {}
    }
}

fn load_usage_ranking_root_aggregates(
    connection: &Connection,
    bounds: RankingFilterBounds<'_>,
    managed: &HashMap<String, SkillKind>,
    include_unmanaged: bool,
    skill_type: Option<SkillUsageRankingSkillType>,
) -> Result<(UsageRankingRootAggregates, SkillUsageCoverage)> {
    let (sql, values) = ranking_filter_sql(
        RankingFilterSqlTemplates {
            no_filter: "
            SELECT
              skill_name,
              runtime_root,
              metadata_json,
              used_at,
              evidence_class,
              evidence_sources_json
            FROM skill_usage_events
            WHERE used_at >= ?1 AND used_at <= ?2
            ",
            workspace_only: "
            SELECT
              skill_name,
              runtime_root,
              metadata_json,
              used_at,
              evidence_class,
              evidence_sources_json
            FROM skill_usage_events
            WHERE used_at >= ?1 AND used_at <= ?2
              AND runtime_root = ?3
            ",
            agent_only: "
            SELECT
              skill_name,
              runtime_root,
              metadata_json,
              used_at,
              evidence_class,
              evidence_sources_json
            FROM skill_usage_events
            WHERE used_at >= ?1 AND used_at <= ?2
              AND agent_id IN ({agent_ids})
            ",
            agent_and_workspace: "
            SELECT
              skill_name,
              runtime_root,
              metadata_json,
              used_at,
              evidence_class,
              evidence_sources_json
            FROM skill_usage_events
            WHERE used_at >= ?1 AND used_at <= ?2
              AND runtime_root = ?3
              AND agent_id IN ({agent_ids})
            ",
        },
        bounds,
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut usage = HashMap::new();
    let mut coverage = SkillUsageCoverage::default();
    let mut source_counts: HashMap<(String, SkillUsageEvidenceClass), usize> = HashMap::new();
    for row in rows {
        let (
            skill_name,
            runtime_root,
            metadata_json,
            used_at,
            evidence_class,
            evidence_sources_json,
        ) = row.map_err(|error| error.to_string())?;
        let evidence_class = parse_usage_evidence_class(&evidence_class)?;
        let source_kind = usage_source_kind_for_event(&skill_name, &runtime_root, &metadata_json);
        let is_managed_regular =
            source_kind == SkillUsageSourceKind::Regular && managed.contains_key(&skill_name);
        if (!include_unmanaged && !is_managed_regular)
            || !usage_ranking_matches_skill_type(&skill_name, source_kind, managed, skill_type)
        {
            continue;
        }
        if coverage
            .earliest_event_at
            .as_ref()
            .is_none_or(|earliest| &used_at < earliest)
        {
            coverage.earliest_event_at = Some(used_at.clone());
        }
        if coverage
            .latest_event_at
            .as_ref()
            .is_none_or(|latest| &used_at > latest)
        {
            coverage.latest_event_at = Some(used_at.clone());
        }
        let evidence_sources =
            parse_usage_evidence_sources(&evidence_sources_json).unwrap_or_else(|_| {
                vec![UsageEvidenceSource {
                    source: usage_event_observation_source(&metadata_json)
                        .unwrap_or("manual")
                        .to_string(),
                    evidence_class,
                }]
            });
        let canonical_source = evidence_sources
            .iter()
            .find(|source| source.evidence_class == evidence_class)
            .map(|source| usage_evidence_source_bucket(&source.source))
            .unwrap_or("manual");
        if usage_evidence_counts_toward_calls(evidence_class) {
            match canonical_source {
                "agent_hook" => {
                    coverage.agent_hook_calls = coverage.agent_hook_calls.saturating_add(1);
                }
                "codex_session_backfill" => {
                    coverage.codex_session_backfill_calls =
                        coverage.codex_session_backfill_calls.saturating_add(1);
                }
                "claude_code_session_backfill" => {
                    coverage.claude_code_session_backfill_calls = coverage
                        .claude_code_session_backfill_calls
                        .saturating_add(1);
                }
                "cursor_session_backfill" => {
                    coverage.cursor_session_backfill_calls =
                        coverage.cursor_session_backfill_calls.saturating_add(1);
                }
                _ => {
                    coverage.other_observed_calls = coverage.other_observed_calls.saturating_add(1);
                }
            }
        }
        for source in evidence_sources {
            *source_counts
                .entry((
                    usage_evidence_source_bucket(&source.source).to_string(),
                    source.evidence_class,
                ))
                .or_default() += 1;
        }
        let summary = usage
            .entry((skill_name, runtime_root, source_kind))
            .or_insert_with(UsageSummary::default);
        match evidence_class {
            SkillUsageEvidenceClass::Confirmed => {
                coverage.confirmed_calls = coverage.confirmed_calls.saturating_add(1);
                update_coverage_timestamp(
                    &mut coverage.earliest_confirmed_at,
                    &mut coverage.latest_confirmed_at,
                    &used_at,
                );
                summary.usage_count = summary.usage_count.saturating_add(1);
                summary.confirmed_count = summary.confirmed_count.saturating_add(1);
                if summary
                    .last_used_at
                    .as_ref()
                    .is_none_or(|last| &used_at > last)
                {
                    summary.last_used_at = Some(used_at);
                }
            }
            SkillUsageEvidenceClass::Inferred => {
                coverage.inferred_calls = coverage.inferred_calls.saturating_add(1);
                update_coverage_timestamp(
                    &mut coverage.earliest_inferred_at,
                    &mut coverage.latest_inferred_at,
                    &used_at,
                );
                summary.usage_count = summary.usage_count.saturating_add(1);
                summary.inferred_count = summary.inferred_count.saturating_add(1);
                if summary
                    .last_used_at
                    .as_ref()
                    .is_none_or(|last| &used_at > last)
                {
                    summary.last_used_at = Some(used_at);
                }
            }
            SkillUsageEvidenceClass::Reference => {
                coverage.history_references = coverage.history_references.saturating_add(1);
                update_coverage_timestamp(
                    &mut coverage.earliest_reference_at,
                    &mut coverage.latest_reference_at,
                    &used_at,
                );
                summary.reference_count = summary.reference_count.saturating_add(1);
                if summary
                    .last_referenced_at
                    .as_ref()
                    .is_none_or(|last| &used_at > last)
                {
                    summary.last_referenced_at = Some(used_at);
                }
            }
        }
    }
    coverage.source_counts = source_counts
        .into_iter()
        .map(
            |((source, evidence_class), count)| SkillUsageEvidenceSourceCount {
                source,
                evidence_class,
                count,
            },
        )
        .collect();
    coverage.source_counts.sort_by(|left, right| {
        left.source.cmp(&right.source).then_with(|| {
            left.evidence_class
                .as_str()
                .cmp(right.evidence_class.as_str())
        })
    });
    Ok((usage, coverage))
}

fn update_coverage_timestamp(
    earliest: &mut Option<String>,
    latest: &mut Option<String>,
    timestamp: &str,
) {
    if earliest
        .as_ref()
        .is_none_or(|current| timestamp < current.as_str())
    {
        *earliest = Some(timestamp.to_string());
    }
    if latest
        .as_ref()
        .is_none_or(|current| timestamp > current.as_str())
    {
        *latest = Some(timestamp.to_string());
    }
}

fn usage_ranking_matches_skill_type(
    skill_name: &str,
    source_kind: SkillUsageSourceKind,
    managed: &HashMap<String, SkillKind>,
    skill_type: Option<SkillUsageRankingSkillType>,
) -> bool {
    match skill_type {
        None => true,
        Some(SkillUsageRankingSkillType::User) => {
            source_kind == SkillUsageSourceKind::Regular
                && managed.get(skill_name) == Some(&SkillKind::User)
        }
        Some(SkillUsageRankingSkillType::Remote) => {
            source_kind == SkillUsageSourceKind::Regular
                && managed.get(skill_name) == Some(&SkillKind::Remote)
        }
        Some(SkillUsageRankingSkillType::System) => source_kind == SkillUsageSourceKind::System,
    }
}

fn usage_event_observation_source(metadata_json: &str) -> Option<&str> {
    let metadata = serde_json::from_str::<serde_json::Value>(metadata_json).ok()?;
    metadata
        .get("source")
        .and_then(|value| value.as_str())
        .map(|source| match source {
            "agent_hook" => "agent_hook",
            "codex_session_backfill" => "codex_session_backfill",
            "claude_code_session_backfill" => "claude_code_session_backfill",
            "cursor_session_backfill" => "cursor_session_backfill",
            _ => "other",
        })
}

fn usage_source_kind_for_event(
    skill_name: &str,
    runtime_root: &str,
    metadata_json: &str,
) -> SkillUsageSourceKind {
    let metadata_kind = serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|metadata| {
            metadata
                .get("skill_source_kind")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    match metadata_kind.as_deref() {
        Some("system") => return SkillUsageSourceKind::System,
        Some("regular") => return SkillUsageSourceKind::Regular,
        _ => {}
    }
    let root = Path::new(runtime_root);
    let regular_present = root.join(skill_name).join("SKILL.md").is_file();
    let system_present = root
        .join(".system")
        .join(skill_name)
        .join("SKILL.md")
        .is_file();
    match (regular_present, system_present) {
        (true, false) | (false, false) => SkillUsageSourceKind::Regular,
        (false, true) => SkillUsageSourceKind::System,
        (true, true) => SkillUsageSourceKind::Unknown,
    }
}

fn unmanaged_regular_skill_source_present(skill_name: &str, roots: &[PathBuf]) -> bool {
    roots
        .iter()
        .any(|root| root.join(skill_name).join("SKILL.md").is_file())
}

fn unmanaged_system_skill_source_present(skill_name: &str, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| {
        root.join(".system")
            .join(skill_name)
            .join("SKILL.md")
            .is_file()
    })
}

fn sorted_usage_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.sort();
    roots.dedup_by(|left, right| usage_runtime_key(left) == usage_runtime_key(right));
    roots
}

fn usage_ranking_source_id(
    skill_name: &str,
    kind: SkillUsageSourceKind,
    roots: &[PathBuf],
) -> String {
    let kind = match kind {
        SkillUsageSourceKind::Regular => "regular",
        SkillUsageSourceKind::System => "system",
        SkillUsageSourceKind::Unknown => "unknown",
    };
    let mut keys = roots
        .iter()
        .map(|root| usage_runtime_key(root))
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    let digest = sha256(&format!("{skill_name}\n{}", keys.join("\n")));
    format!("{skill_name}:{kind}:{}", &digest[..12])
}

struct RankingFilterSqlTemplates<'a> {
    no_filter: &'a str,
    workspace_only: &'a str,
    agent_only: &'a str,
    agent_and_workspace: &'a str,
}

struct RankingFilterBounds<'a> {
    range_start: &'a str,
    range_end: &'a str,
    agent_ids: &'a [String],
    workspace_root: Option<&'a str>,
}

fn ranking_filter_sql(
    templates: RankingFilterSqlTemplates<'_>,
    bounds: RankingFilterBounds<'_>,
) -> (String, Vec<String>) {
    match (bounds.agent_ids.is_empty(), bounds.workspace_root) {
        (true, None) => (
            templates.no_filter.to_string(),
            vec![bounds.range_start.to_string(), bounds.range_end.to_string()],
        ),
        (true, Some(workspace_root)) => (
            templates.workspace_only.to_string(),
            vec![
                bounds.range_start.to_string(),
                bounds.range_end.to_string(),
                workspace_root.to_string(),
            ],
        ),
        (false, None) => {
            let placeholders = sql_in_placeholders(3, bounds.agent_ids.len());
            let mut values = vec![bounds.range_start.to_string(), bounds.range_end.to_string()];
            values.extend(bounds.agent_ids.iter().cloned());
            (
                templates.agent_only.replace("{agent_ids}", &placeholders),
                values,
            )
        }
        (false, Some(workspace_root)) => {
            let placeholders = sql_in_placeholders(4, bounds.agent_ids.len());
            let mut values = vec![
                bounds.range_start.to_string(),
                bounds.range_end.to_string(),
                workspace_root.to_string(),
            ];
            values.extend(bounds.agent_ids.iter().cloned());
            (
                templates
                    .agent_and_workspace
                    .replace("{agent_ids}", &placeholders),
                values,
            )
        }
    }
}

fn sql_in_placeholders(start: usize, count: usize) -> String {
    (0..count)
        .map(|offset| format!("?{}", start + offset))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn normalize_usage_timestamp(value: Option<&str>) -> Result<String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(current_rfc3339_timestamp());
    };
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| {
            timestamp
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, false)
        })
        .map_err(|error| format!("Invalid usage timestamp: {error}"))
}

pub(crate) fn normalize_usage_agent_id(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    let canonical = match normalized.as_str() {
        "codex" | "codex-app" | "codex-cli" | "agents" => "codex",
        "claude" | "claude-code" | "claude-code-cli" => "claude-code",
        other => other,
    };
    if canonical.is_empty()
        || canonical
            .chars()
            .any(|character| !matches!(character, 'a'..='z' | '0'..='9' | '-' | '_'))
    {
        return Err(format!("Invalid usage agent id: {value}"));
    }
    Ok(canonical.to_string())
}

pub(crate) fn usage_ranking_agent_filter_ids(agent_id: &str) -> Vec<String> {
    match agent_id {
        "codex" => vec!["codex".to_string(), "agents".to_string()],
        "claude-code" => vec!["claude-code".to_string(), "claude".to_string()],
        other => vec![other.to_string()],
    }
}

pub(crate) fn normalize_usage_runtime_root(path: PathBuf) -> Result<PathBuf> {
    let expanded = expand_home(path);
    if !expanded.is_absolute() {
        return Err("Usage runtime root must be an absolute path.".to_string());
    }
    Ok(fs::canonicalize(&expanded).unwrap_or(expanded))
}

pub(crate) fn normalize_usage_event_id(value: Option<String>) -> Result<Option<String>> {
    value
        .map(|event_id| {
            let event_id = event_id.trim().to_string();
            if event_id.is_empty() {
                Err("Usage event id cannot be empty.".to_string())
            } else {
                Ok(event_id)
            }
        })
        .transpose()
}

pub(crate) fn normalize_usage_metadata(value: Option<serde_json::Value>) -> Result<String> {
    let metadata = value.unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    if !metadata.is_object() {
        return Err("Usage metadata must be a JSON object.".to_string());
    }
    if let Some(key) = usage_metadata_content_key(&metadata) {
        return Err(format!(
            "Usage metadata cannot include content field: {key}"
        ));
    }

    let metadata_json = serde_json::to_string(&metadata).map_err(|error| error.to_string())?;
    if metadata_json.len() > MAX_USAGE_METADATA_JSON_BYTES {
        return Err(format!(
            "Usage metadata must be at most {MAX_USAGE_METADATA_JSON_BYTES} bytes."
        ));
    }
    Ok(metadata_json)
}

fn merge_usage_source_identity(existing: &str, incoming: &str) -> Result<Option<String>> {
    let mut existing = serde_json::from_str::<serde_json::Value>(existing)
        .map_err(|error| format!("Invalid stored usage metadata: {error}"))?;
    let incoming = serde_json::from_str::<serde_json::Value>(incoming)
        .map_err(|error| format!("Invalid usage metadata: {error}"))?;
    let existing_object = existing
        .as_object_mut()
        .ok_or_else(|| "Stored usage metadata must be a JSON object.".to_string())?;
    if existing_object.contains_key("skill_source_kind") {
        return Ok(None);
    }
    let Some(source_kind) = incoming.get("skill_source_kind").cloned() else {
        return Ok(None);
    };
    existing_object.insert("skill_source_kind".to_string(), source_kind);
    normalize_usage_metadata(Some(existing)).map(Some)
}

fn classify_usage_evidence(
    metadata_json: &str,
    trusted_generated_source: bool,
) -> (SkillUsageEvidenceClass, String) {
    if !trusted_generated_source {
        return (SkillUsageEvidenceClass::Reference, "manual".to_string());
    }
    let metadata = serde_json::from_str::<serde_json::Value>(metadata_json).ok();
    let source = metadata
        .as_ref()
        .and_then(|metadata| metadata.get("source"))
        .and_then(|value| value.as_str())
        .filter(|source| is_trusted_generated_usage_source(source))
        .unwrap_or("manual")
        .to_string();
    let evidence_class = match source.as_str() {
        "agent_hook" | "cursor_agent_transcript_read" => SkillUsageEvidenceClass::Confirmed,
        "codex_session_backfill" => SkillUsageEvidenceClass::Inferred,
        "claude_code_session_backfill"
            if metadata
                .as_ref()
                .and_then(|metadata| metadata.get("evidence_signal"))
                .and_then(|value| value.as_str())
                .is_some_and(|signal| {
                    matches!(signal, "native_skill_tool" | "native_skill_command")
                }) =>
        {
            SkillUsageEvidenceClass::Confirmed
        }
        _ => SkillUsageEvidenceClass::Reference,
    };
    (evidence_class, source)
}

fn usage_evidence_source_bucket(source: &str) -> &'static str {
    match source {
        "agent_hook" => "agent_hook",
        "codex_session_backfill" => "codex_session_backfill",
        "claude_code_session_backfill" => "claude_code_session_backfill",
        "cursor_session_backfill" => "cursor_session_backfill",
        "cursor_agent_transcript_read" => "cursor_agent_transcript_read",
        _ => "manual",
    }
}

fn parse_usage_evidence_class(value: &str) -> Result<SkillUsageEvidenceClass> {
    match value {
        "confirmed" => Ok(SkillUsageEvidenceClass::Confirmed),
        "inferred" => Ok(SkillUsageEvidenceClass::Inferred),
        "reference" => Ok(SkillUsageEvidenceClass::Reference),
        _ => Err(format!("Invalid stored usage evidence class: {value}")),
    }
}

fn usage_evidence_rank(evidence_class: SkillUsageEvidenceClass) -> u8 {
    match evidence_class {
        SkillUsageEvidenceClass::Reference => 0,
        SkillUsageEvidenceClass::Inferred => 1,
        SkillUsageEvidenceClass::Confirmed => 2,
    }
}

fn usage_evidence_counts_toward_calls(evidence_class: SkillUsageEvidenceClass) -> bool {
    evidence_class != SkillUsageEvidenceClass::Reference
}

fn merge_usage_evidence_sources(
    existing_json: &str,
    incoming: &str,
    incoming_class: SkillUsageEvidenceClass,
) -> Result<String> {
    const MAX_EVIDENCE_SOURCES: usize = 8;
    let mut sources = parse_usage_evidence_sources(existing_json)?;
    if let Some(existing) = sources.iter_mut().find(|item| item.source == incoming) {
        if usage_evidence_rank(incoming_class) > usage_evidence_rank(existing.evidence_class) {
            existing.evidence_class = incoming_class;
        }
    } else if sources.len() < MAX_EVIDENCE_SOURCES {
        sources.push(UsageEvidenceSource {
            source: incoming.to_string(),
            evidence_class: incoming_class,
        });
    }
    serde_json::to_string(&sources).map_err(|error| error.to_string())
}

fn parse_usage_evidence_sources(value: &str) -> Result<Vec<UsageEvidenceSource>> {
    let raw = serde_json::from_str::<Vec<serde_json::Value>>(value)
        .map_err(|error| format!("Invalid stored usage evidence sources: {error}"))?;
    let mut sources = Vec::new();
    for item in raw {
        match item {
            serde_json::Value::String(source) => sources.push(UsageEvidenceSource {
                source,
                evidence_class: SkillUsageEvidenceClass::Reference,
            }),
            serde_json::Value::Object(object) => {
                let source = object
                    .get("source")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        "Invalid stored usage evidence source: missing source.".to_string()
                    })?;
                let evidence_class = object
                    .get("evidence_class")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        "Invalid stored usage evidence source: missing evidence_class.".to_string()
                    })
                    .and_then(parse_usage_evidence_class)?;
                sources.push(UsageEvidenceSource {
                    source: source.to_string(),
                    evidence_class,
                });
            }
            _ => {
                return Err("Invalid stored usage evidence source entry.".to_string());
            }
        }
    }
    Ok(sources)
}

pub(crate) fn normalize_usage_prompt_excerpt(value: Option<&str>) -> Option<String> {
    let stripped = strip_skill_blocks(value?);
    let stripped = strip_skill_markdown_links(&stripped);
    let compact = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }

    let mut chars = compact.chars();
    let mut excerpt = chars
        .by_ref()
        .take(MAX_USAGE_PROMPT_EXCERPT_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        excerpt.push_str("...");
    }
    Some(excerpt)
}

pub(crate) fn strip_skill_blocks(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("<skill>") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + "<skill>".len()..];
        let Some(end) = after_start.find("</skill>") else {
            return output;
        };
        remaining = &after_start[end + "</skill>".len()..];
    }

    output.push_str(remaining);
    output
}

pub(crate) fn strip_skill_markdown_links(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("[$") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start..];
        let Some(label_end) = after_start.find("](") else {
            output.push_str(after_start);
            return output;
        };
        let Some(link_end) = after_start[label_end + 2..].find(')') else {
            output.push_str(after_start);
            return output;
        };
        remaining = &after_start[label_end + 2 + link_end + 1..];
    }

    output.push_str(remaining);
    output
}

pub(crate) fn usage_metadata_content_key(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => {
            for (key, nested) in object {
                let normalized_key = key.to_ascii_lowercase();
                if USAGE_METADATA_CONTENT_KEYS
                    .iter()
                    .any(|content_key| content_key == &normalized_key)
                {
                    return Some(key.clone());
                }
                if let Some(content_key) = usage_metadata_content_key(nested) {
                    return Some(content_key);
                }
            }
            None
        }
        serde_json::Value::Array(values) => values.iter().find_map(usage_metadata_content_key),
        _ => None,
    }
}

pub(crate) fn usage_runtime_key(path: &Path) -> String {
    let expanded = expand_home(path.to_path_buf());
    fs::canonicalize(&expanded)
        .unwrap_or(expanded)
        .to_string_lossy()
        .to_string()
}

pub(crate) fn skill_symlink_target_path(skill: &Skill) -> Option<PathBuf> {
    if skill.is_symlink {
        Some(skill.real_path.clone())
    } else {
        None
    }
}

pub(crate) fn skill_usage_runtime_keys(skill: &Skill, paths: &ManagedPaths) -> Vec<String> {
    let mut keys = Vec::new();

    if let Some(source_root) = skill.source_root.as_ref() {
        push_unique_usage_runtime_key(&mut keys, source_root);
    }

    if skill.is_symlink {
        for runtime_root in symlink_target_usage_roots(&skill.real_path, paths) {
            push_unique_usage_runtime_key(&mut keys, &runtime_root);
        }
    }

    keys
}

pub(crate) fn symlink_target_usage_roots(real_path: &Path, paths: &ManagedPaths) -> Vec<PathBuf> {
    let real_path = fs::canonicalize(real_path).unwrap_or_else(|_| real_path.to_path_buf());
    let mut roots = Vec::new();

    if let Some(runtime_root) = runtime_skill_root_for_path(&real_path) {
        roots.push(runtime_root);
    }
    if is_under_path(&real_path, &paths.user_skills_root) {
        roots.push(paths.user_skills_root.clone());
    }
    if is_under_path(&real_path, &paths.remote_skills_root) {
        for ancestor in real_path.ancestors() {
            if ancestor.file_name().and_then(|name| name.to_str()) == Some("versions")
                && is_under_path(ancestor, &paths.remote_skills_root)
            {
                roots.push(ancestor.to_path_buf());
                break;
            }
        }
    }

    dedupe_runtime_roots(roots)
}

pub(crate) fn runtime_skill_root_for_path(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        if !is_runtime_skill_root(ancestor) {
            continue;
        }
        return Some(fs::canonicalize(ancestor).unwrap_or_else(|_| ancestor.to_path_buf()));
    }
    None
}

pub(crate) fn push_unique_usage_runtime_key(keys: &mut Vec<String>, root: &Path) {
    let key = usage_runtime_key(root);
    if !keys.contains(&key) {
        keys.push(key);
    }
}

pub(crate) fn load_usage_by_skill(database_path: &Path) -> Result<HashMap<String, UsageSummary>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, SUM(usage_count), MAX(last_used_at)
            FROM skill_usage_stats
            GROUP BY skill_name
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let usage_count: i64 = row.get(1)?;
            Ok((
                row.get::<_, String>(0)?,
                UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: row.get(2)?,
                    ..UsageSummary::default()
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut usage = HashMap::new();
    for row in rows {
        let (skill_name, summary) = row.map_err(|error| error.to_string())?;
        usage.insert(skill_name, summary);
    }
    enrich_call_evidence_by_skill(&connection, &mut usage)?;
    enrich_reference_usage_by_skill(&connection, &mut usage)?;
    Ok(usage)
}

pub(crate) fn load_usage_by_runtime(database_path: &Path) -> Result<HashMap<String, UsageSummary>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT runtime_root, SUM(usage_count), MAX(last_used_at)
            FROM skill_usage_stats
            GROUP BY runtime_root
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let usage_count: i64 = row.get(1)?;
            Ok((
                row.get::<_, String>(0)?,
                UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: row.get(2)?,
                    ..UsageSummary::default()
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut usage = HashMap::new();
    for row in rows {
        let (runtime_root, summary) = row.map_err(|error| error.to_string())?;
        usage.insert(runtime_root, summary);
    }
    enrich_call_evidence_by_runtime(&connection, &mut usage)?;
    enrich_reference_usage_by_runtime(&connection, &mut usage)?;
    Ok(usage)
}

pub(crate) fn load_usage_by_skill_runtime(
    database_path: &Path,
) -> Result<HashMap<(String, String), UsageSummary>> {
    let connection = open_database(database_path).map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, runtime_root, usage_count, last_used_at
            FROM skill_usage_stats
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let usage_count: i64 = row.get(2)?;
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: row.get(3)?,
                    ..UsageSummary::default()
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut usage = HashMap::new();
    for row in rows {
        let (key, summary) = row.map_err(|error| error.to_string())?;
        usage.insert(key, summary);
    }
    enrich_call_evidence_by_skill_runtime(&connection, &mut usage)?;
    enrich_reference_usage_by_skill_runtime(&connection, &mut usage)?;
    Ok(usage)
}

fn enrich_call_evidence_by_skill(
    connection: &Connection,
    usage: &mut HashMap<String, UsageSummary>,
) -> Result<()> {
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, evidence_class, COUNT(*)
            FROM skill_usage_events
            WHERE evidence_class IN ('confirmed', 'inferred')
            GROUP BY skill_name, evidence_class
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                usize::try_from(row.get::<_, i64>(2)?.max(0)).unwrap_or_default(),
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (key, evidence_class, count) = row.map_err(|error| error.to_string())?;
        apply_call_evidence_count(usage.entry(key).or_default(), &evidence_class, count)?;
    }
    Ok(())
}

fn enrich_call_evidence_by_runtime(
    connection: &Connection,
    usage: &mut HashMap<String, UsageSummary>,
) -> Result<()> {
    let mut statement = connection
        .prepare(
            "
            SELECT runtime_root, evidence_class, COUNT(*)
            FROM skill_usage_events
            WHERE evidence_class IN ('confirmed', 'inferred')
            GROUP BY runtime_root, evidence_class
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                usize::try_from(row.get::<_, i64>(2)?.max(0)).unwrap_or_default(),
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (key, evidence_class, count) = row.map_err(|error| error.to_string())?;
        apply_call_evidence_count(usage.entry(key).or_default(), &evidence_class, count)?;
    }
    Ok(())
}

fn enrich_call_evidence_by_skill_runtime(
    connection: &Connection,
    usage: &mut HashMap<(String, String), UsageSummary>,
) -> Result<()> {
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, runtime_root, evidence_class, COUNT(*)
            FROM skill_usage_events
            WHERE evidence_class IN ('confirmed', 'inferred')
            GROUP BY skill_name, runtime_root, evidence_class
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                row.get::<_, String>(2)?,
                usize::try_from(row.get::<_, i64>(3)?.max(0)).unwrap_or_default(),
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (key, evidence_class, count) = row.map_err(|error| error.to_string())?;
        apply_call_evidence_count(usage.entry(key).or_default(), &evidence_class, count)?;
    }
    Ok(())
}

fn apply_call_evidence_count(
    summary: &mut UsageSummary,
    evidence_class: &str,
    count: usize,
) -> Result<()> {
    match parse_usage_evidence_class(evidence_class)? {
        SkillUsageEvidenceClass::Confirmed => summary.confirmed_count = count,
        SkillUsageEvidenceClass::Inferred => summary.inferred_count = count,
        SkillUsageEvidenceClass::Reference => {}
    }
    Ok(())
}

fn enrich_reference_usage_by_skill(
    connection: &Connection,
    usage: &mut HashMap<String, UsageSummary>,
) -> Result<()> {
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, COUNT(*), MAX(used_at)
            FROM skill_usage_events
            WHERE evidence_class = 'reference'
            GROUP BY skill_name
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                usize::try_from(row.get::<_, i64>(1)?.max(0)).unwrap_or_default(),
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (key, reference_count, last_referenced_at) = row.map_err(|error| error.to_string())?;
        let summary = usage.entry(key).or_default();
        summary.reference_count = reference_count;
        summary.last_referenced_at = last_referenced_at;
    }
    Ok(())
}

fn enrich_reference_usage_by_runtime(
    connection: &Connection,
    usage: &mut HashMap<String, UsageSummary>,
) -> Result<()> {
    let mut statement = connection
        .prepare(
            "
            SELECT runtime_root, COUNT(*), MAX(used_at)
            FROM skill_usage_events
            WHERE evidence_class = 'reference'
            GROUP BY runtime_root
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                usize::try_from(row.get::<_, i64>(1)?.max(0)).unwrap_or_default(),
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (key, reference_count, last_referenced_at) = row.map_err(|error| error.to_string())?;
        let summary = usage.entry(key).or_default();
        summary.reference_count = reference_count;
        summary.last_referenced_at = last_referenced_at;
    }
    Ok(())
}

fn enrich_reference_usage_by_skill_runtime(
    connection: &Connection,
    usage: &mut HashMap<(String, String), UsageSummary>,
) -> Result<()> {
    let mut statement = connection
        .prepare(
            "
            SELECT skill_name, runtime_root, COUNT(*), MAX(used_at)
            FROM skill_usage_events
            WHERE evidence_class = 'reference'
            GROUP BY skill_name, runtime_root
            ",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                usize::try_from(row.get::<_, i64>(2)?.max(0)).unwrap_or_default(),
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (key, reference_count, last_referenced_at) = row.map_err(|error| error.to_string())?;
        let summary = usage.entry(key).or_default();
        summary.reference_count = reference_count;
        summary.last_referenced_at = last_referenced_at;
    }
    Ok(())
}

type ExistingUsageEventRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
);

fn generated_usage_event_runtime(
    connection: &Connection,
    skill_name: &str,
    canonical_agent_id: &str,
    event_id: Option<&str>,
    metadata_json: &str,
    trusted_generated_source: bool,
) -> Result<Option<String>> {
    let Some(event_id) = event_id else {
        return Ok(None);
    };
    let generated_source = serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|metadata| {
            metadata
                .get("source")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .is_some_and(|source| is_trusted_generated_usage_source(&source));
    if !trusted_generated_source || !generated_source {
        return Ok(None);
    }

    let agent_ids = usage_ranking_agent_filter_ids(canonical_agent_id);
    let placeholders = sql_in_placeholders(1, agent_ids.len());
    let mut values = agent_ids;
    values.push(event_id.to_string());
    values.push(skill_name.to_string());
    values.push(canonical_agent_id.to_string());
    connection
        .query_row(
            &format!(
                "
                SELECT runtime_root, metadata_json
                FROM skill_usage_events
                WHERE agent_id IN ({placeholders})
                  AND event_id = ?{}
                  AND skill_name = ?{}
                ORDER BY
                  CASE WHEN agent_id = ?{} THEN 0 ELSE 1 END,
                  recorded_at ASC,
                  runtime_root ASC
                LIMIT 1
                ",
                values.len() - 2,
                values.len() - 1,
                values.len(),
            ),
            rusqlite::params_from_iter(values.iter()),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())
        .map(|existing| {
            existing.and_then(|(runtime_root, existing_metadata_json)| {
                let existing_generated =
                    serde_json::from_str::<serde_json::Value>(&existing_metadata_json)
                        .ok()
                        .and_then(|metadata| {
                            metadata
                                .get("source")
                                .and_then(|value| value.as_str())
                                .map(str::to_string)
                        })
                        .is_some_and(|source| is_trusted_generated_usage_source(&source));
                existing_generated.then_some(runtime_root)
            })
        })
}

fn is_trusted_generated_usage_source(source: &str) -> bool {
    matches!(
        source,
        "agent_hook"
            | "codex_session_backfill"
            | "claude_code_session_backfill"
            | "cursor_session_backfill"
            | "cursor_agent_transcript_read"
    )
}

fn canonicalize_runtime_usage_agent_aliases(
    connection: &Connection,
    canonical_agent_id: &str,
    runtime_root: &str,
) -> Result<()> {
    let aliases = usage_ranking_agent_filter_ids(canonical_agent_id)
        .into_iter()
        .filter(|alias| alias != canonical_agent_id)
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        return Ok(());
    }
    let placeholders = sql_in_placeholders(1, aliases.len());
    let mut exists_values = aliases.clone();
    exists_values.push(runtime_root.to_string());
    let has_legacy_events: bool = connection
        .query_row(
            &format!(
                "
                SELECT EXISTS(
                  SELECT 1
                  FROM skill_usage_events
                  WHERE agent_id IN ({placeholders})
                    AND runtime_root = ?{}
                )
                ",
                exists_values.len(),
            ),
            rusqlite::params_from_iter(exists_values.iter()),
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !has_legacy_events {
        return Ok(());
    }

    for alias in &aliases {
        let duplicate_events = {
            let mut statement = connection
                .prepare(
                    "
                    SELECT
                      legacy.event_id,
                      legacy.evidence_class,
                      legacy.evidence_sources_json
                    FROM skill_usage_events AS legacy
                    WHERE legacy.agent_id = ?1
                      AND legacy.runtime_root = ?2
                      AND legacy.event_id IS NOT NULL
                      AND EXISTS (
                        SELECT 1
                        FROM skill_usage_events AS canonical
                        WHERE canonical.agent_id = ?3
                          AND canonical.runtime_root = legacy.runtime_root
                          AND canonical.event_id = legacy.event_id
                      )
                    ",
                )
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map(params![alias, runtime_root, canonical_agent_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|error| error.to_string())?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        };
        for (event_id, alias_class, alias_sources_json) in duplicate_events {
            let (canonical_class, canonical_sources_json) = connection
                .query_row(
                    "
                    SELECT evidence_class, evidence_sources_json
                    FROM skill_usage_events
                    WHERE agent_id = ?1 AND runtime_root = ?2 AND event_id = ?3
                    ",
                    params![canonical_agent_id, runtime_root, &event_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map_err(|error| error.to_string())?;
            let mut merged_sources = canonical_sources_json;
            for source in parse_usage_evidence_sources(&alias_sources_json).unwrap_or_default() {
                merged_sources = merge_usage_evidence_sources(
                    &merged_sources,
                    &source.source,
                    source.evidence_class,
                )?;
            }
            let alias_class = parse_usage_evidence_class(&alias_class)?;
            let canonical_class = parse_usage_evidence_class(&canonical_class)?;
            let merged_class =
                if usage_evidence_rank(alias_class) > usage_evidence_rank(canonical_class) {
                    alias_class
                } else {
                    canonical_class
                };
            connection
                .execute(
                    "
                    UPDATE skill_usage_events
                    SET evidence_class = ?1,
                        evidence_sources_json = ?2
                    WHERE agent_id = ?3 AND runtime_root = ?4 AND event_id = ?5
                    ",
                    params![
                        merged_class.as_str(),
                        merged_sources,
                        canonical_agent_id,
                        runtime_root,
                        event_id
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        connection
            .execute(
                "
                DELETE FROM skill_usage_events
                WHERE agent_id = ?1
                  AND runtime_root = ?2
                  AND event_id IS NOT NULL
                  AND EXISTS (
                    SELECT 1
                    FROM skill_usage_events AS canonical
                    WHERE canonical.agent_id = ?3
                      AND canonical.runtime_root = skill_usage_events.runtime_root
                      AND canonical.event_id = skill_usage_events.event_id
                  )
                ",
                params![alias, runtime_root, canonical_agent_id],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "
                UPDATE skill_usage_events
                SET agent_id = ?1
                WHERE agent_id = ?2 AND runtime_root = ?3
                ",
                params![canonical_agent_id, alias, runtime_root],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "
                DELETE FROM skill_usage_stats
                WHERE agent_id = ?1 AND runtime_root = ?2
                ",
                params![alias, runtime_root],
            )
            .map_err(|error| error.to_string())?;
    }
    connection
        .execute(
            "
            DELETE FROM skill_usage_stats
            WHERE agent_id = ?1 AND runtime_root = ?2
            ",
            params![canonical_agent_id, runtime_root],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "
            INSERT INTO skill_usage_stats (
              skill_name,
              agent_id,
              runtime_root,
              usage_count,
              last_used_at
            )
            SELECT
              skill_name,
              agent_id,
              runtime_root,
              COUNT(*),
              MAX(used_at)
            FROM skill_usage_events
            WHERE agent_id = ?1
              AND runtime_root = ?2
              AND evidence_class IN ('confirmed', 'inferred')
            GROUP BY skill_name, agent_id, runtime_root
            ",
            params![canonical_agent_id, runtime_root],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn find_existing_usage_event(
    connection: &Connection,
    canonical_agent_id: &str,
    runtime_root: &str,
    event_id: &str,
) -> Result<Option<ExistingUsageEventRow>> {
    let canonical = connection
        .query_row(
            "
            SELECT
              agent_id,
              used_at,
              recorded_at,
              prompt_excerpt,
              metadata_json,
              evidence_class,
              evidence_sources_json
            FROM skill_usage_events
            WHERE agent_id = ?1 AND runtime_root = ?2 AND event_id = ?3
            ",
            params![canonical_agent_id, runtime_root, event_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if canonical.is_some() {
        // Merge any leftover legacy alias evidence before removing duplicate rows.
        let aliases = usage_ranking_agent_filter_ids(canonical_agent_id);
        for alias in aliases {
            if alias == canonical_agent_id {
                continue;
            }
            if let Some((alias_class, alias_sources)) = connection
                .query_row(
                    "
                    SELECT evidence_class, evidence_sources_json
                    FROM skill_usage_events
                    WHERE agent_id = ?1 AND runtime_root = ?2 AND event_id = ?3
                    ",
                    params![alias, runtime_root, event_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(|error| error.to_string())?
            {
                let mut merged = canonical.clone().expect("canonical event exists");
                let mut sources = parse_usage_evidence_sources(&alias_sources).unwrap_or_default();
                for source in sources.drain(..) {
                    merged.6 = merge_usage_evidence_sources(
                        &merged.6,
                        &source.source,
                        source.evidence_class,
                    )?;
                }
                let alias_class = parse_usage_evidence_class(&alias_class)?;
                let canonical_class = parse_usage_evidence_class(&merged.5)?;
                if usage_evidence_rank(alias_class) > usage_evidence_rank(canonical_class) {
                    merged.5 = alias_class.as_str().to_string();
                }
                connection
                    .execute(
                        "
                        UPDATE skill_usage_events
                        SET evidence_class = ?1,
                            evidence_sources_json = ?2
                        WHERE agent_id = ?3 AND runtime_root = ?4 AND event_id = ?5
                        ",
                        params![
                            &merged.5,
                            &merged.6,
                            canonical_agent_id,
                            runtime_root,
                            event_id
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            connection
                .execute(
                    "
                    DELETE FROM skill_usage_events
                    WHERE agent_id = ?1 AND runtime_root = ?2 AND event_id = ?3
                    ",
                    params![alias, runtime_root, event_id],
                )
                .map_err(|error| error.to_string())?;
        }
        return connection
            .query_row(
                "
                SELECT
                  agent_id,
                  used_at,
                  recorded_at,
                  prompt_excerpt,
                  metadata_json,
                  evidence_class,
                  evidence_sources_json
                FROM skill_usage_events
                WHERE agent_id = ?1 AND runtime_root = ?2 AND event_id = ?3
                ",
                params![canonical_agent_id, runtime_root, event_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string());
    }

    let aliases = usage_ranking_agent_filter_ids(canonical_agent_id)
        .into_iter()
        .filter(|alias| alias != canonical_agent_id)
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        return Ok(None);
    }
    let placeholders = sql_in_placeholders(1, aliases.len());
    let mut values = aliases;
    values.push(runtime_root.to_string());
    values.push(event_id.to_string());
    connection
        .query_row(
            &format!(
                "
                SELECT
                  agent_id,
                  used_at,
                  recorded_at,
                  prompt_excerpt,
                  metadata_json,
                  evidence_class,
                  evidence_sources_json
                FROM skill_usage_events
                WHERE agent_id IN ({placeholders})
                  AND runtime_root = ?{}
                  AND event_id = ?{}
                ",
                values.len() - 1,
                values.len(),
            ),
            rusqlite::params_from_iter(values.iter()),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())
}

pub(crate) fn merge_legacy_usage_stat_into_canonical(
    connection: &Connection,
    skill_name: &str,
    legacy_agent_id: &str,
    canonical_agent_id: &str,
    runtime_root: &str,
) -> Result<()> {
    let legacy = connection
        .query_row(
            "
            SELECT usage_count, last_used_at
            FROM skill_usage_stats
            WHERE skill_name = ?1 AND agent_id = ?2 AND runtime_root = ?3
            ",
            params![skill_name, legacy_agent_id, runtime_root],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((legacy_count, legacy_last_used_at)) = legacy else {
        return Ok(());
    };

    let canonical = connection
        .query_row(
            "
            SELECT usage_count, last_used_at
            FROM skill_usage_stats
            WHERE skill_name = ?1 AND agent_id = ?2 AND runtime_root = ?3
            ",
            params![skill_name, canonical_agent_id, runtime_root],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    if let Some((canonical_count, canonical_last_used_at)) = canonical {
        let last_used_at = if legacy_last_used_at > canonical_last_used_at {
            legacy_last_used_at
        } else {
            canonical_last_used_at
        };
        connection
            .execute(
                "
                UPDATE skill_usage_stats
                SET usage_count = ?1,
                    last_used_at = ?2,
                    updated_at = CURRENT_TIMESTAMP
                WHERE skill_name = ?3 AND agent_id = ?4 AND runtime_root = ?5
                ",
                params![
                    canonical_count.saturating_add(legacy_count),
                    last_used_at,
                    skill_name,
                    canonical_agent_id,
                    runtime_root,
                ],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "
                DELETE FROM skill_usage_stats
                WHERE skill_name = ?1 AND agent_id = ?2 AND runtime_root = ?3
                ",
                params![skill_name, legacy_agent_id, runtime_root],
            )
            .map_err(|error| error.to_string())?;
    } else {
        connection
            .execute(
                "
                UPDATE skill_usage_stats
                SET agent_id = ?1,
                    updated_at = CURRENT_TIMESTAMP
                WHERE skill_name = ?2 AND agent_id = ?3 AND runtime_root = ?4
                ",
                params![
                    canonical_agent_id,
                    skill_name,
                    legacy_agent_id,
                    runtime_root
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn load_usage_stat_for_key(
    connection: &Connection,
    skill_name: &str,
    agent_id: &str,
    runtime_root: &str,
) -> Result<UsageSummary> {
    connection
        .query_row(
            "
            SELECT usage_count, last_used_at
            FROM skill_usage_stats
            WHERE skill_name = ?1 AND agent_id = ?2 AND runtime_root = ?3
            ",
            params![skill_name, agent_id, runtime_root],
            |row| {
                let usage_count: i64 = row.get(0)?;
                Ok(UsageSummary {
                    usage_count: usize::try_from(usage_count.max(0)).unwrap_or_default(),
                    last_used_at: Some(row.get(1)?),
                    ..UsageSummary::default()
                })
            },
        )
        .optional()
        .map(|usage| usage.unwrap_or_default())
        .map_err(|error| error.to_string())
}
