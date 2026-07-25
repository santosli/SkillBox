import React from 'react';
import { HardDriveDownload, RefreshCw } from 'lucide-react';
import { formatOperationTimestamp } from '../remoteSkills.js';
import { numberOrZero } from '../skills.js';
import {
  formatUsageRankingRank,
  usageRankingAgentOptions,
  usageRankingBarPercent,
  usageRankingKindTone,
  usageRankingRangeLabel,
  usageRankingRangeOptions,
  usageRankingSkillTypeOptions,
  usageRankingScopeLabel,
  usageRankingTopRows,
  usageRankingWorkspaceOptions
} from '../usageRankings.js';
import { Badge, PageFrame, PageTitleRow } from './common.jsx';
import { DashboardStatusNotice } from './dashboard.jsx';

export function UsageRankingsPage({
  backfilling = false,
  error,
  filters,
  importingSkillName = '',
  loading,
  notice = '',
  ranking,
  usageHooks,
  workspaces,
  onSyncHistories,
  onDismissNotice,
  onFilters,
  onImportSkill,
  onOpenSettings,
  onOpenSkill,
  onRefresh
}) {
  const rows = ranking.rows || [];
  const topRows = usageRankingTopRows(rows);
  const maxUsageCount = topRows.reduce(
    (highest, row) => Math.max(highest, numberOrZero(row.usageCount)),
    0
  );
  const agentOptions = usageRankingAgentOptions(usageHooks);
  const workspaceOptions = usageRankingWorkspaceOptions(workspaces);
  const rangeLabel = usageRankingRangeLabel(filters.range);
  const updateFilters = (patch) => onFilters({ ...filters, ...patch });
  const hasObservedCalls = numberOrZero(ranking.totalObservedCalls) > 0;
  const busy = loading || backfilling || Boolean(importingSkillName);

  return (
    <PageFrame ariaLabel="Rankings">
      <PageTitleRow
        title="Rankings"
        count={rows.length}
        actions={(
          <div className="pageTitleActions">
            <button
              className="button secondary"
              disabled={busy}
              type="button"
              onClick={onSyncHistories}
            >
              <HardDriveDownload aria-hidden="true" />
              {backfilling ? 'Syncing...' : 'Sync histories'}
            </button>
            <button className="button secondary" disabled={busy} type="button" onClick={onRefresh}>
              <RefreshCw aria-hidden="true" />
              {loading ? 'Loading...' : 'Refresh'}
            </button>
          </div>
        )}
      />

      <section className="usageRankingPanel" aria-label="Local skill usage rankings">
        <div className="usageRankingControls">
          <div className="usageRankingRangeField">
            <span className="usageRankingSelectLabel" id="usage-ranking-range-label">
              Time range
            </span>
            <div
              className="dashboardTypeTabs usageRankingRanges"
              role="group"
              aria-labelledby="usage-ranking-range-label"
            >
              {usageRankingRangeOptions.map((option) => (
                <button
                  aria-pressed={filters.range === option.id}
                  className={filters.range === option.id ? 'active' : ''}
                  disabled={busy}
                  key={option.id}
                  type="button"
                  onClick={() => updateFilters({ range: option.id })}
                >
                  <span>{option.label}</span>
                </button>
              ))}
            </div>
          </div>

          <label className="usageRankingSelect">
            <span className="usageRankingSelectLabel">Skill type</span>
            <select
              disabled={busy}
              value={filters.skillType}
              onChange={(event) => updateFilters({ skillType: event.target.value })}
            >
              <option value="">All types</option>
              {usageRankingSkillTypeOptions.map((option) => (
                <option key={option.id} value={option.id}>{option.label}</option>
              ))}
            </select>
          </label>

          <label className="usageRankingSelect">
            <span className="usageRankingSelectLabel">Agent</span>
            <select
              disabled={busy}
              value={filters.agentId}
              onChange={(event) => updateFilters({ agentId: event.target.value })}
            >
              <option value="">All agents</option>
              {agentOptions.map((option) => (
                <option key={option.id} value={option.id}>{option.label}</option>
              ))}
            </select>
          </label>

          <label className="usageRankingSelect workspace">
            <span className="usageRankingSelectLabel">Workspace</span>
            <select
              disabled={busy}
              value={filters.workspaceRoot}
              onChange={(event) => updateFilters({ workspaceRoot: event.target.value })}
            >
              <option value="">All workspaces</option>
              {workspaceOptions.map((option) => (
                <option key={option.id} value={option.id}>{option.label} · {option.detail}</option>
              ))}
            </select>
          </label>
        </div>

        <UsageCoverageSummary coverage={ranking.coverage} />

        {error ? <div className="panelNotice notice">{error}</div> : null}
        {notice ? (
          <DashboardStatusNotice message={notice} onDismiss={onDismissNotice} />
        ) : null}

        {loading ? (
          <div className="usageRankingLoading" role="status">
            <span className="inlineSpinner" aria-hidden="true" />
            Updating rankings...
          </div>
        ) : backfilling ? (
          <div className="usageRankingLoading" role="status">
            <span className="inlineSpinner" aria-hidden="true" />
            Scanning local agent histories...
          </div>
        ) : !hasObservedCalls ? (
          <div className="emptyState dashboardEmptyState historyEmptyState">
            <strong>No locally observed skill calls in this range</strong>
            <span>
              Sync local agent histories, enable a trusted usage hook, or choose a wider time range.
            </span>
            <div className="emptyStateActions">
              <button className="button primary" type="button" onClick={onSyncHistories}>
                Sync histories
              </button>
              <button className="button secondary" type="button" onClick={onOpenSettings}>
                Open usage hook settings
              </button>
            </div>
          </div>
        ) : (
          <>
            <section className="usageRankingSection" aria-label="Most locally observed skills">
              <div className="usageRankingSectionHeader">
                <h2>Most locally observed</h2>
                <span>Current range: {rangeLabel}</span>
              </div>
              <div className="usageRankingTopGrid">
                {topRows.map((row) => (
                  <UsageRankingTopCard
                    key={row.sourceId || `${row.skillName}:${row.system ? 'system' : 'regular'}`}
                    maxUsageCount={maxUsageCount}
                    row={row}
                    onOpenSkill={onOpenSkill}
                  />
                ))}
              </div>
            </section>

            <section className="usageRankingSection" aria-label="Full ranking">
              <div className="usageRankingSectionHeader">
                <h2>Full ranking</h2>
                <span>Includes skills not imported into SkillBox</span>
              </div>
              <div className="usageRankingTableWrap">
                <table className="usageRankingTable">
                  <caption className="srOnly">Skills ranked by locally observed calls</caption>
                  <thead>
                    <tr>
                      <th scope="col">Rank</th>
                      <th scope="col">Skill</th>
                      <th scope="col">Calls</th>
                      <th scope="col">Last observed</th>
                      <th scope="col">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => {
                      const rowId = row.sourceId || row.skillName;
                      const importing = importingSkillName === rowId;
                      return (
                      <tr key={rowId} className={row.managed ? undefined : 'unmanaged'}>
                        <td>
                          <span className="usageRankingPosition">{formatUsageRankingRank(row.rank)}</span>
                        </td>
                        <td>
                          <div className="usageRankingSkill">
                            {row.managed ? (
                              <button type="button" onClick={() => onOpenSkill(row.skillName)}>
                                {row.skillName}
                              </button>
                            ) : (
                              <strong
                                title={
                                  row.system
                                    ? 'Codex system skill — not importable into SkillBox'
                                    : row.sourceKind === 'unknown'
                                      ? 'Historical usage could not be attributed to a regular or System source'
                                    : row.sourceMissing
                                      ? 'Previously observed, but the local skill source is gone'
                                      : 'Observed locally but not imported into SkillBox'
                                }
                              >
                                {row.skillName}
                              </strong>
                            )}
                            <Badge tone={usageRankingKindTone(row)}>
                              {usageRankingScopeLabel(row)}
                            </Badge>
                          </div>
                        </td>
                        <td><strong className="usageRankingCalls">{row.usageCount}</strong></td>
                        <td>
                          {row.lastUsedAt ? (
                            <time dateTime={row.lastUsedAt}>{formatOperationTimestamp(row.lastUsedAt)}</time>
                          ) : (
                            <span className="usageRankingNever">Not observed</span>
                          )}
                        </td>
                        <td>
                          <div className="usageRankingActions">
                            {row.managed ? (
                              <button
                                className="button primary compactAction"
                                disabled={busy}
                                type="button"
                                onClick={() => onOpenSkill(row.skillName)}
                              >
                                Detail
                              </button>
                            ) : row.system || row.sourceKind === 'unknown' || row.sourceMissing ? null : (
                              <button
                                className="button primary compactAction"
                                disabled={busy}
                                type="button"
                                onClick={() => onImportSkill(row)}
                              >
                                {importing ? 'Preparing...' : 'Import'}
                              </button>
                            )}
                          </div>
                        </td>
                      </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </section>
          </>
        )}
      </section>
    </PageFrame>
  );
}

function UsageRankingTopCard({ maxUsageCount, onOpenSkill, row }) {
  const barPercent = usageRankingBarPercent(row.usageCount, maxUsageCount);
  const isLeader = numberOrZero(row.rank) === 1;
  const lastLabel = row.lastUsedAt
    ? `Last ${formatOperationTimestamp(row.lastUsedAt)}`
    : 'Not observed';

  return (
    <article
      className={isLeader ? 'usageRankingTopCard leader' : 'usageRankingTopCard'}
      aria-label={`${formatUsageRankingRank(row.rank)} ${row.skillName}`}
    >
      <div className="usageRankingTopCardHeader">
        <span className="usageRankingTopCardRank">{formatUsageRankingRank(row.rank)}</span>
        <Badge tone={usageRankingKindTone(row)}>
          {usageRankingScopeLabel(row)}
        </Badge>
      </div>

      {row.managed ? (
        <button
          className="usageRankingTopCardName"
          type="button"
          onClick={() => onOpenSkill(row.skillName)}
        >
          {row.skillName}
        </button>
      ) : (
        <strong
          className="usageRankingTopCardName"
          title={
            row.system
              ? 'Codex system skill — not importable into SkillBox'
              : row.sourceKind === 'unknown'
                ? 'Historical usage could not be attributed to a regular or System source'
              : row.sourceMissing
                ? 'Previously observed, but the local skill source is gone'
                : 'Observed locally but not imported into SkillBox'
          }
        >
          {row.skillName}
        </strong>
      )}

      <div className="usageRankingTopCardMeta">
        <strong>{row.usageCount} calls</strong>
        {row.lastUsedAt ? (
          <time dateTime={row.lastUsedAt}>{lastLabel}</time>
        ) : (
          <span>{lastLabel}</span>
        )}
      </div>

      <div
        aria-hidden="true"
        className="usageRankingTopCardBar"
        style={{ '--usage-bar-width': `${barPercent}%` }}
      />
    </article>
  );
}

function UsageCoverageSummary({ coverage = {} }) {
  const earliest = coverage.earliestEventAt || '';
  const latest = coverage.latestEventAt || '';
  const eventWindow = earliest && latest
    ? `Earliest ${formatOperationTimestamp(earliest)} · Latest ${formatOperationTimestamp(latest)}`
    : 'No local events in this range';

  return (
    <section className="usageCoverageSummary" aria-label="Local usage data coverage">
      <div className="usageCoverageHeader">
        <div>
          <strong>Local data coverage</strong>
          <span>Auditable events only; counts use the current filters.</span>
        </div>
        <span className="usageCoverageWindow">{eventWindow}</span>
      </div>
      <dl className="usageCoverageMetrics">
        <div>
          <dt>Hook observations</dt>
          <dd>{numberOrZero(coverage.agentHookCalls)}</dd>
        </div>
        <div>
          <dt>Codex history observations</dt>
          <dd>{numberOrZero(coverage.codexSessionBackfillCalls)}</dd>
        </div>
        <div>
          <dt>Claude Code history observations</dt>
          <dd>{numberOrZero(coverage.claudeCodeSessionBackfillCalls)}</dd>
        </div>
        <div>
          <dt>Cursor history observations</dt>
          <dd>{numberOrZero(coverage.cursorSessionBackfillCalls)}</dd>
        </div>
        <div>
          <dt>Other local observations</dt>
          <dd>{numberOrZero(coverage.otherObservedCalls)}</dd>
        </div>
        <div>
          <dt>History sessions scanned</dt>
          <dd>
            {numberOrZero(coverage.scannedCodexSessionFiles)
              + numberOrZero(coverage.scannedClaudeCodeSessionFiles)
              + numberOrZero(coverage.scannedCursorSessions)}
          </dd>
        </div>
      </dl>
      <p>
        Provider-reported runs use separate metrics and are never added to locally observed calls
        or this ranking.
      </p>
    </section>
  );
}
