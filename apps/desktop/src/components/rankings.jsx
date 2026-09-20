import React, { useState } from 'react';
import { AlertTriangle, ChevronDown, HardDriveDownload, Info, RefreshCw } from 'lucide-react';
import { formatOperationTimestamp } from '../remoteSkills.js';
import { numberOrZero } from '../skills.js';
import {
  formatUsageRankingRank,
  usageRankingAgentOptions,
  usageRankingBodyMode,
  usageRankingFiltersLocked,
  usageRankingKindTone,
  usageRankingRangeOptions,
  usageRankingSkillTypeOptions,
  usageRankingScopeLabel,
  usageRankingWorkspaceOptions,
  buildUsageHeatmap,
  formatUsageTrendDate,
  usageHeatmapLayout,
  usageHeatmapLevelColor,
  usageHeatmapWeekdayRows,
  usageStatsTableRows,
  visibleUsageHeatmapMonths,
} from '../usageRankings.js';
import {
  usageBackfillProgressDetail,
  usageBackfillProgressLabel,
  usageBackfillProgressPercent,
  normalizeUsageBackfillProgress
} from '../usageBackfillProgress.js';
import { Badge, PageFrame, PageTitleRow } from './common.jsx';
import { DashboardStatusNotice } from './dashboard.jsx';

export function UsageRankingsPage({
  backfilling = false,
  backfillProgress = null,
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
  const [coverageExpanded, setCoverageExpanded] = useState(false);
  const [selectedDate, setSelectedDate] = useState('');
  const rows = ranking.rows || [];
  const heatmap = buildUsageHeatmap(ranking.daily || []);
  const selectedDay = (heatmap.days || []).find((day) => day.date === selectedDate) || null;
  const tableRows = usageStatsTableRows(rows, selectedDay);
  const statsCallTotal = selectedDay
    ? numberOrZero(selectedDay.totalCalls)
    : numberOrZero(ranking.totalCalls);
  const statsCallLabel = `${statsCallTotal} ${statsCallTotal === 1 ? 'call' : 'calls'}`;
  const agentOptions = usageRankingAgentOptions(usageHooks);
  const workspaceOptions = usageRankingWorkspaceOptions(workspaces);
  const updateFilters = (patch) => onFilters({ ...filters, ...patch });
  const hasObservedCalls = numberOrZero(ranking.totalCalls) > 0;
  const showRangeEmpty = !selectedDay && !hasObservedCalls;
  const filtersLocked = usageRankingFiltersLocked({
    backfilling,
    importingSkillName
  });
  const bodyMode = usageRankingBodyMode(ranking, { loading, backfilling });
  const busy = loading || backfilling || Boolean(importingSkillName);

  return (
    <PageFrame ariaLabel="Usage">
      <PageTitleRow
        title="Usage"
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

      <section className="usageRankingPanel" aria-label="Local skill usage">
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
                  disabled={filtersLocked}
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
              disabled={filtersLocked}
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
              disabled={filtersLocked}
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
              disabled={filtersLocked}
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

        <UsageCoverageDisclosure
          coverage={ranking.coverage}
          expanded={coverageExpanded}
          totalCalls={ranking.totalCalls}
          totalHistoryReferences={ranking.totalHistoryReferences}
          warning={Boolean(error)}
          onToggle={() => setCoverageExpanded((current) => !current)}
        />

        {error ? <div className="panelNotice notice">{error}</div> : null}
        {notice ? (
          <DashboardStatusNotice message={notice} onDismiss={onDismissNotice} />
        ) : null}

        {bodyMode === 'progress' ? (
          <UsageHistoryScanProgress progress={backfillProgress} />
        ) : bodyMode === 'loading' ? (
          <div className="usageRankingLoading" role="status">
            <span className="inlineSpinner" aria-hidden="true" />
            Updating usage...
          </div>
        ) : (
          <div
            aria-busy={loading ? 'true' : undefined}
            className="usageRankingSnapshot"
          >
            <section className="usageRankingSection usageHeatmapSection" aria-label="Call activity">
              <div className="usageRankingSectionHeader">
                <h2>Call activity</h2>
                <span>Past year{loading ? ' · Updating' : ''}</span>
              </div>
              <UsageCallHeatmap
                heatmap={heatmap}
                selectedDate={selectedDate}
                onSelectDate={setSelectedDate}
              />
            </section>

            {showRangeEmpty ? (
          <div className="emptyState dashboardEmptyState historyEmptyState">
            <strong>No calls in this range</strong>
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
            <section className="usageRankingSection" aria-label="Usage 统计">
              <div className="usageRankingSectionHeader">
                <h2>Usage 统计</h2>
                <span>
                  {selectedDay
                    ? `${formatUsageTrendDate(selectedDay.date, { includeYear: true })} · ${statsCallLabel}`
                    : statsCallLabel}
                </span>
              </div>
              {tableRows.length === 0 ? (
                <p className="usageHeatmapEmptyState">No calls on this day.</p>
              ) : (
              <div className="usageRankingTableWrap">
                <table className="usageRankingTable">
                  <caption className="srOnly">
                    {selectedDay
                      ? `Skill calls on ${formatUsageTrendDate(selectedDay.date, { includeYear: true })}`
                      : 'Skills ranked by calls'}
                  </caption>
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
                    {tableRows.map((row) => {
                      const rowId = row.sourceId || `${row.sourceKind}:${row.skillName}`;
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
                            ) : row.system || row.sourceKind === 'unknown' || row.sourceMissing || !row.sourceId ? null : (
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
              )}
            </section>
          </>
            )}
          </div>
        )}
      </section>
    </PageFrame>
  );
}

function UsageCallHeatmap({ heatmap, selectedDate = '', onSelectDate }) {
  const [tooltip, setTooltip] = useState(null);
  const weeks = heatmap.weeks || [];
  const layout = usageHeatmapLayout(weeks.length);
  const months = visibleUsageHeatmapMonths(heatmap.months, layout, weeks.length);
  const weekdays = usageHeatmapWeekdayRows();
  const step = layout.cell + layout.gap;

  const showDayTooltip = (event, label) => {
    const rect = event.currentTarget.getBoundingClientRect();
    setTooltip({
      label,
      x: rect.left + rect.width / 2,
      y: rect.top
    });
  };

  if (weeks.length === 0) {
    return <p className="usageHeatmapEmptyState">No daily calls in this year.</p>;
  }

  return (
    <div className="usageHeatmap">
      <div className="usageHeatmapScroll">
        <div
          className="usageHeatmapCanvas"
          style={{
            '--usage-heatmap-cell': `${layout.cell}px`,
            '--usage-heatmap-gap': `${layout.gap}px`,
            '--usage-heatmap-weekday': `${layout.weekdayWidth}px`,
            '--usage-heatmap-weekday-gap': `${layout.weekdayGap}px`,
            width: layout.canvasWidth
          }}
        >
          <div className="usageHeatmapMonths" aria-hidden="true">
            {months.map((month) => (
              <span
                key={`${month.label}:${month.weekIndex}`}
                className="usageHeatmapMonth"
                style={{
                  left: month.weekIndex * step
                }}
              >
                {month.label}
              </span>
            ))}
          </div>
          <div className="usageHeatmapBody">
            <div className="usageHeatmapWeekdays" aria-hidden="true">
              {weekdays.map((weekday) => (
                <span
                  className="usageHeatmapWeekday"
                  key={weekday.label}
                  style={{ top: weekday.row * step }}
                >
                  {weekday.label}
                </span>
              ))}
            </div>
            <div className="usageHeatmapGrid" role="grid" aria-label="Calls by day over the past year">
              {weeks.map((week, weekIndex) => (
                <div className="usageHeatmapWeek" key={week[0]?.date || weekIndex} role="row">
                  {week.map((day) => {
                    const selectedDay = selectedDate === day.date;
                    const label = day.inRange
                      ? `${formatUsageTrendDate(day.date, { includeYear: true })} · ${day.totalCalls} ${day.totalCalls === 1 ? 'call' : 'calls'}`
                      : formatUsageTrendDate(day.date, { includeYear: true });
                    return (
                      <button
                        aria-disabled={day.inRange ? undefined : 'true'}
                        aria-label={label}
                        aria-pressed={day.inRange ? selectedDay : undefined}
                        className={[
                          'usageHeatmapCell',
                          day.inRange ? '' : 'isOutside',
                          selectedDay ? 'isSelected' : ''
                        ].filter(Boolean).join(' ')}
                        disabled={!day.inRange}
                        key={day.date}
                        role="gridcell"
                        style={{ background: usageHeatmapLevelColor(day.level) }}
                        type="button"
                        onBlur={() => setTooltip(null)}
                        onClick={() => onSelectDate?.(selectedDay ? '' : day.date)}
                        onFocus={(event) => showDayTooltip(event, label)}
                        onMouseEnter={(event) => showDayTooltip(event, label)}
                        onMouseLeave={() => setTooltip(null)}
                      />
                    );
                  })}
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
      {tooltip ? (
        <div
          className="usageHeatmapTooltip"
          role="tooltip"
          style={{ left: tooltip.x, top: tooltip.y }}
        >
          {tooltip.label}
        </div>
      ) : null}
    </div>
  );
}

function UsageCoverageDisclosure({
  coverage = {},
  expanded,
  onToggle,
  totalCalls,
  totalHistoryReferences,
  warning
}) {
  const earliest = coverage.earliestEventAt || '';
  const latest = coverage.latestEventAt || '';
  const eventWindow = earliest && latest
    ? `${formatOperationTimestamp(earliest)}–${formatOperationTimestamp(latest)}`
    : 'No local observations in this range';
  const detailedEventWindow = earliest && latest
    ? `Earliest ${formatOperationTimestamp(earliest)} · Latest ${formatOperationTimestamp(latest)}`
    : 'No local observations in this range';

  return (
    <div className="usageCoverageDisclosure">
      <button
        aria-controls="usage-coverage-details"
        aria-expanded={expanded}
        className="usageCoverageToggle"
        type="button"
        onClick={onToggle}
      >
        <Info aria-hidden="true" className="usageCoverageInfoIcon" />
        <span className="usageCoverageSummaryText">
          <span>{numberOrZero(totalCalls)} locally observed calls</span>
          <span aria-hidden="true">·</span>
          <span>{numberOrZero(totalHistoryReferences)} history references</span>
          <span aria-hidden="true">·</span>
          <span className="usageCoverageWindow">{eventWindow}</span>
        </span>
        {warning ? (
          <span
            className="usageCoverageWarning"
            title="Coverage or ranking refresh needs attention"
          >
            <AlertTriangle aria-hidden="true" />
            Warning
          </span>
        ) : null}
        <span className="usageCoverageAction">
          {expanded ? 'Hide coverage' : 'View coverage'}
          <ChevronDown
            aria-hidden="true"
            className={expanded ? 'usageCoverageChevron expanded' : 'usageCoverageChevron'}
          />
        </span>
      </button>

      <section
        aria-label="Local usage data coverage"
        className="usageCoverageDetails"
        hidden={!expanded}
        id="usage-coverage-details"
      >
        <div className="usageCoverageDetailsHeader">
          <h2>Local data coverage</h2>
          <div>
            <span>Auditable events only; counts use the current filters.</span>
            <span className="usageCoverageWindow">{detailedEventWindow}</span>
          </div>
        </div>
        <dl className="usageCoverageMetrics">
          <div>
            <dt>Confirmed calls</dt>
            <dd>{numberOrZero(coverage.confirmedCalls)}</dd>
          </div>
          <div>
            <dt>Inferred calls</dt>
            <dd>{numberOrZero(coverage.inferredCalls)}</dd>
          </div>
          <div>
            <dt>History references</dt>
            <dd>{numberOrZero(coverage.historyReferences)}</dd>
          </div>
          <div>
            <dt>Codex transcript files</dt>
            <dd>{numberOrZero(coverage.scannedCodexSessionFiles)}</dd>
          </div>
          <div>
            <dt>Codex turns scanned</dt>
            <dd>{numberOrZero(coverage.scannedCodexTurns)}</dd>
          </div>
          <div>
            <dt>Claude Code transcript files</dt>
            <dd>{numberOrZero(coverage.scannedClaudeCodeSessionFiles)}</dd>
          </div>
          <div>
            <dt>Cursor transcript files</dt>
            <dd>{numberOrZero(coverage.scannedCursorTranscriptFiles)}</dd>
          </div>
          <div>
            <dt>Cursor state DB sessions</dt>
            <dd>{numberOrZero(coverage.scannedCursorSessions)}</dd>
          </div>
        </dl>
        {coverage.sourceCounts?.length ? (
          <div className="usageCoverageSources" aria-label="Usage evidence sources">
            {coverage.sourceCounts.map((source) => (
              <span key={`${source.source}:${source.evidenceClass}`}>
                {usageEvidenceSourceLabel(source.source)} · {source.evidenceClass} · {source.count}
              </span>
            ))}
          </div>
        ) : null}
        <p>
          Calls combine locally confirmed executions with high-confidence inferred invocations
          found in structured local histories. History references are explicit mentions that do
          not prove an invocation and are never added to Calls. Neither metric is Codex or Claude
          account analytics; provider-reported runs remain separate. Codex does not expose a
          stable local provider total, so these auditable Calls may still undercount actual usage.
        </p>
      </section>
    </div>
  );
}

function UsageHistoryScanProgress({ progress }) {
  const normalized = normalizeUsageBackfillProgress(progress);
  const percent = usageBackfillProgressPercent(normalized);
  return (
    <div className="usageRankingLoading" role="status" aria-live="polite" aria-atomic="true">
      <span className="inlineSpinner" aria-hidden="true" />
      <div className="usageRankingProgressCopy">
        <strong>{usageBackfillProgressLabel(normalized)}</strong>
        <span>{usageBackfillProgressDetail(normalized)}</span>
        {percent == null ? null : (
          <div
            className="usageRankingProgressTrack"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
          >
            <span className="usageRankingProgressFill" style={{ width: `${percent}%` }} />
          </div>
        )}
      </div>
    </div>
  );
}

function usageEvidenceSourceLabel(source = '') {
  const labels = {
    agent_hook: 'Agent hooks',
    codex_session_backfill: 'Codex structured history',
    claude_code_session_backfill: 'Claude Code Skill tools',
    cursor_agent_transcript_read: 'Cursor transcript reads',
    cursor_session_backfill: 'Cursor state references',
    manual: 'Manual records'
  };
  return labels[source] || String(source || 'Other').replaceAll('_', ' ');
}
