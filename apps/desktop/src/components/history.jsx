import React from 'react';
import { RefreshCw } from 'lucide-react';
import {
  groupHistoryEntriesByDay,
  historyRowSubtitle,
  operationStatusTone
} from '../historyEntries.js';
import { formatOperationTimestamp } from '../remoteSkills.js';
import { compactPath, numberOrZero } from '../skills.js';
import {
  usageRankingAgentOptions,
  usageRankingRangeOptions,
  usageRankingWorkspaceOptions
} from '../usageRankings.js';
import { Badge, PageTitleRow } from './common.jsx';

export function HistoryPage({
  error,
  filter,
  history,
  ranking,
  rankingFilters,
  rankingLoading,
  status,
  usageHooks,
  workspaces,
  onFilter,
  onOpenSettings,
  onOpenSkill,
  onRankingFilters,
  onRefresh
}) {
  const entries = history.entries || [];
  const rankingRows = ranking.rows || [];
  const tabs = [
    {
      id: 'all',
      label: 'All',
      count: numberOrZero(history.skillUsageCount) + numberOrZero(history.operationCount)
    },
    { id: 'skill_usage', label: 'Skill calls', count: numberOrZero(history.skillUsageCount) },
    { id: 'operation', label: 'Operations', count: numberOrZero(history.operationCount) },
    { id: 'rankings', label: 'Rankings', count: rankingRows.length }
  ];
  const filteredEntries =
    filter === 'all' ? entries : entries.filter((entry) => entry.kind === filter);
  const groupedEntries = groupHistoryEntriesByDay(filteredEntries);
  const isLoading = status === 'loading_history';
  const visibleCount = filter === 'rankings' ? rankingRows.length : filteredEntries.length;

  return (
    <section className="dashboardFrame historyFrame" aria-label="History">
      {error ? <div className="notice">{error}</div> : null}
      <PageTitleRow
        title="History"
        count={visibleCount}
        actions={(
          <button className="button secondary" disabled={isLoading} type="button" onClick={onRefresh}>
            <RefreshCw aria-hidden="true" />
            {isLoading ? 'Loading...' : 'Refresh'}
          </button>
        )}
      />

      <div className="dashboardFilterBar pageTypeFilterBar" aria-label="History filters">
        <div className="dashboardTypeTabs historyTypeTabs" role="tablist" aria-label="History type">
          {tabs.map((tab) => (
            <button
              aria-selected={filter === tab.id}
              className={filter === tab.id ? 'active' : ''}
              key={tab.id}
              role="tab"
              type="button"
              onClick={() => onFilter(tab.id)}
            >
              <span>{tab.label}</span>
              <small>{tab.count}</small>
            </button>
          ))}
        </div>
      </div>

      {filter === 'rankings' ? (
        <UsageRankingPanel
          filters={rankingFilters}
          loading={rankingLoading}
          ranking={ranking}
          usageHooks={usageHooks}
          workspaces={workspaces}
          onFilters={onRankingFilters}
          onOpenSettings={onOpenSettings}
          onOpenSkill={onOpenSkill}
        />
      ) : filteredEntries.length > 0 ? (
        <div className="historyTimeline" aria-label="History entries">
          {groupedEntries.map((group) => (
            <section className="historyDayBlock" key={group.key} aria-label={`${group.label} history`}>
              <div className="historyDayHeader">
                <h2>{group.label}</h2>
                <span>{group.entries.length}</span>
              </div>
              <div className="historyDayRows">
                {group.entries.map((entry) => (
                  <HistoryRow entry={entry} key={`${entry.kind}:${entry.id}`} />
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        <div className="emptyState dashboardEmptyState historyEmptyState">
          <strong>No history yet</strong>
          <span>Skill calls and SkillBox operations will appear here.</span>
        </div>
      )}
    </section>
  );
}

function UsageRankingPanel({
  filters,
  loading,
  ranking,
  usageHooks,
  workspaces,
  onFilters,
  onOpenSettings,
  onOpenSkill
}) {
  const rows = ranking.rows || [];
  const agentOptions = usageRankingAgentOptions(usageHooks);
  const workspaceOptions = usageRankingWorkspaceOptions(workspaces);
  const updateFilters = (patch) => onFilters({ ...filters, ...patch });

  return (
    <section className="usageRankingPanel" aria-label="Local skill usage rankings">
      <div className="usageRankingControls">
        <div className="usageRankingRanges" role="group" aria-label="Ranking time range">
          {usageRankingRangeOptions.map((option) => (
            <button
              aria-pressed={filters.range === option.id}
              className={filters.range === option.id ? 'active' : ''}
              disabled={loading}
              key={option.id}
              type="button"
              onClick={() => updateFilters({ range: option.id })}
            >
              {option.label}
            </button>
          ))}
        </div>

        <label className="usageRankingSelect">
          <span>Agent</span>
          <select
            disabled={loading}
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
          <span>Workspace</span>
          <select
            disabled={loading}
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

      <div className="usageRankingCoverage" role="note">
        <strong>{numberOrZero(ranking.totalObservedCalls)} observed calls</strong>
        <span>
          Local records from enabled and trusted hooks only. Zero means no call was observed,
          not that a skill was never used.
        </span>
      </div>

      {loading ? (
        <div className="usageRankingLoading" role="status">
          <span className="inlineSpinner" aria-hidden="true" />
          Updating rankings...
        </div>
      ) : numberOrZero(ranking.totalObservedCalls) === 0 ? (
        <div className="emptyState dashboardEmptyState historyEmptyState">
          <strong>No observed skill calls in this range</strong>
          <span>Enable and trust a usage hook, or choose a wider time range.</span>
          <button className="button secondary" type="button" onClick={onOpenSettings}>
            Open usage hook settings
          </button>
        </div>
      ) : (
        <div className="usageRankingTableWrap">
          <table className="usageRankingTable">
            <caption className="srOnly">Skills ranked by locally observed calls</caption>
            <thead>
              <tr>
                <th scope="col">Rank</th>
                <th scope="col">Skill</th>
                <th scope="col">Calls</th>
                <th scope="col">Last observed</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.skillName}>
                  <td><span className="usageRankingPosition">#{row.rank}</span></td>
                  <td>
                    <div className="usageRankingSkill">
                      {row.managed ? (
                        <button type="button" onClick={() => onOpenSkill(row.skillName)}>
                          {row.skillName}
                        </button>
                      ) : (
                        <strong>{row.skillName}</strong>
                      )}
                      {row.kind ? <Badge tone="slate">{row.kind}</Badge> : null}
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
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function HistoryRow({ entry }) {
  const timestamp = formatOperationTimestamp(entry.timestamp);
  const timestampParts = timestamp.split(' ');
  const timestampTime = timestampParts.length > 1 ? timestampParts.slice(1).join(' ') : timestamp;
  const isUsage = entry.kind === 'skill_usage';
  const badgeLabel = isUsage ? 'Call' : entry.status || 'operation';
  const badgeTone = isUsage ? 'blue' : operationStatusTone(entry.status);
  const details = isUsage
    ? [entry.agentId, compactPath(entry.runtimeRoot)].filter(Boolean)
    : [entry.operationType, entry.actor, entry.entityName].filter(Boolean);
  const rowSubtitle = historyRowSubtitle(entry, isUsage);

  return (
    <article className={isUsage ? 'historyRow usage' : 'historyRow operation'}>
      <div className="historyRowTimeRail">
        {timestamp ? (
          <time className="historyRowTimestamp" dateTime={entry.timestamp}>
            <strong>{timestampTime}</strong>
          </time>
        ) : null}
      </div>
      <div className="historyRowTitle">
        <strong>{entry.title || entry.skillName || entry.operationType || 'History event'}</strong>
        <Badge tone={badgeTone}>{badgeLabel}</Badge>
      </div>
      <div className="historyRowMain">
        <div className="historyRowMeta">
          {details.map((detail) => (
            <span key={detail}>{detail}</span>
          ))}
        </div>
        {rowSubtitle ? <p>{rowSubtitle}</p> : null}
        {isUsage && entry.promptExcerpt ? (
          <div className="historyRowPrompt">
            <span>Prompt</span>
            <p>{entry.promptExcerpt}</p>
          </div>
        ) : null}
        {entry.error ? <small>{entry.error}</small> : null}
      </div>
    </article>
  );
}
