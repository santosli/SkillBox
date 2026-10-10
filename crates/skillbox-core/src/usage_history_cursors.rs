use crate::*;

pub const USAGE_HISTORY_AUTO_SYNC_INTERVAL_SECS: u64 = 3_600;
pub(crate) const USAGE_HISTORY_CURSOR_PROVIDER_CODEX: &str = "codex-session";
pub(crate) const USAGE_HISTORY_CURSOR_PROVIDER_CLAUDE: &str = "claude-code-session";
pub(crate) const USAGE_HISTORY_CURSOR_PROVIDER_CURSOR_STATE: &str = "cursor-state";
pub(crate) const USAGE_HISTORY_CURSOR_PROVIDER_CURSOR_TRANSCRIPT: &str = "cursor-transcript";
const LAST_SYNC_PREFERENCE: &str = "usage_history_last_sync_at";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HistoryFileStamp {
    pub size: i64,
    pub mtime_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryFileScanDecision {
    SkipUnchanged,
    Scan(HistoryFileStamp),
}

pub fn usage_history_sync_status(managed_root: impl AsRef<Path>) -> Result<UsageHistorySyncStatus> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    let last_synced_at = read_string_preference(&paths.database_path, LAST_SYNC_PREFERENCE)?;
    let due = usage_history_sync_is_due(last_synced_at.as_deref(), Utc::now());
    Ok(UsageHistorySyncStatus {
        last_synced_at,
        interval_seconds: USAGE_HISTORY_AUTO_SYNC_INTERVAL_SECS,
        due,
    })
}

pub fn mark_usage_history_sync_completed(managed_root: impl AsRef<Path>) -> Result<()> {
    let paths = ensure_managed_layout(managed_root.as_ref().to_path_buf())?;
    write_string_preference(
        &paths.database_path,
        LAST_SYNC_PREFERENCE,
        &Utc::now().to_rfc3339(),
    )
}

pub(crate) fn usage_history_sync_is_due(last_synced_at: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(last_synced_at) = last_synced_at else {
        return true;
    };
    let Ok(parsed) = DateTime::parse_from_rfc3339(last_synced_at) else {
        return true;
    };
    let elapsed = now
        .signed_duration_since(parsed.with_timezone(&Utc))
        .num_seconds();
    elapsed >= USAGE_HISTORY_AUTO_SYNC_INTERVAL_SECS as i64
}

pub(crate) fn history_file_stamp(path: &Path) -> Result<HistoryFileStamp> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Unable to inspect {}: {error}", path.display()))?;
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    Ok(HistoryFileStamp {
        size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        mtime_ms,
    })
}

pub(crate) fn history_file_scan_decision(
    connection: &Connection,
    provider: &str,
    path: &Path,
    incremental: bool,
) -> Result<HistoryFileScanDecision> {
    let stamp = history_file_stamp(path)?;
    if incremental && history_file_cursor_matches(connection, provider, path, stamp)? {
        return Ok(HistoryFileScanDecision::SkipUnchanged);
    }
    Ok(HistoryFileScanDecision::Scan(stamp))
}

pub(crate) fn take_history_file_scan(
    connection: &Connection,
    provider: &str,
    path: &Path,
    incremental: bool,
    unchanged_files: &mut usize,
) -> Result<Option<HistoryFileStamp>> {
    match history_file_scan_decision(connection, provider, path, incremental)? {
        HistoryFileScanDecision::SkipUnchanged => {
            *unchanged_files = unchanged_files.saturating_add(1);
            Ok(None)
        }
        HistoryFileScanDecision::Scan(stamp) => Ok(Some(stamp)),
    }
}

pub(crate) fn upsert_history_file_cursor(
    connection: &Connection,
    provider: &str,
    path: &Path,
    stamp: HistoryFileStamp,
) -> Result<()> {
    connection
        .execute(
            "
            INSERT INTO usage_history_file_cursors (
              provider, path, size, mtime_ms, scanned_at
            )
            VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            ON CONFLICT(provider, path) DO UPDATE SET
              size = excluded.size,
              mtime_ms = excluded.mtime_ms,
              scanned_at = excluded.scanned_at
            ",
            params![
                provider,
                path.to_string_lossy().as_ref(),
                stamp.size,
                stamp.mtime_ms
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn write_nondecreasing_u32_preference(
    database_path: &Path,
    key: &str,
    value: u32,
    incremental: bool,
) -> Result<()> {
    let previous = read_u32_preference(database_path, key)?.unwrap_or(0);
    let next = if incremental {
        previous.max(value)
    } else {
        value
    };
    write_u32_preference(database_path, key, next)
}

pub(crate) fn persist_history_coverage_count(
    database_path: &Path,
    key: &str,
    value: usize,
    incremental: bool,
) -> Result<()> {
    write_nondecreasing_u32_preference(
        database_path,
        key,
        u32::try_from(value).unwrap_or(u32::MAX),
        incremental,
    )
}

fn history_file_cursor_matches(
    connection: &Connection,
    provider: &str,
    path: &Path,
    stamp: HistoryFileStamp,
) -> Result<bool> {
    let stored: Option<(i64, i64)> = connection
        .query_row(
            "
            SELECT size, mtime_ms
            FROM usage_history_file_cursors
            WHERE provider = ?1 AND path = ?2
            ",
            params![provider, path.to_string_lossy().as_ref()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(stored == Some((stamp.size, stamp.mtime_ms)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_history_sync_is_due_without_prior_sync() {
        assert!(usage_history_sync_is_due(None, Utc::now()));
    }

    #[test]
    fn usage_history_sync_is_due_after_interval() {
        let now = DateTime::parse_from_rfc3339("2026-09-21T03:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!usage_history_sync_is_due(
            Some("2026-09-21T02:00:01Z"),
            now
        ));
        assert!(usage_history_sync_is_due(Some("2026-09-21T01:59:59Z"), now));
    }

    #[test]
    fn usage_history_sync_status_starts_due_and_clears_after_mark() {
        let root = std::env::temp_dir().join(format!(
            "skillbox-usage-history-sync-status-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let managed = root.join("SkillBox");
        let status = usage_history_sync_status(&managed).unwrap();
        assert!(status.due);
        assert_eq!(
            status.interval_seconds,
            USAGE_HISTORY_AUTO_SYNC_INTERVAL_SECS
        );
        assert!(status.last_synced_at.is_none());
        mark_usage_history_sync_completed(&managed).unwrap();
        let status = usage_history_sync_status(&managed).unwrap();
        assert!(!status.due);
        assert!(status.last_synced_at.is_some());
        let _ = fs::remove_dir_all(root);
    }
}
