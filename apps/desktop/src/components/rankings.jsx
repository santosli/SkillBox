import React from 'react';
import { RefreshCw } from 'lucide-react';
import { formatOperationTimestamp } from '../remoteSkills.js';
import { numberOrZero } from '../skills.js';
import {
  usageRankingAgentOptions,
  usageRankingRangeOptions,
  usageRankingWorkspaceOptions
} from '../usageRankings.js';
import { Badge, PageTitleRow } from './common.jsx';

export function UsageRankingsPage({
  error,
  filters,
  loading,
  ranking,
  usageHooks,
  workspaces,
  onFilters,
  onOpenSettings,
  onOpenSkill,
  onRefresh
}) {
  const rows = ranking.rows || [];
  const agentOptions = usageRankingAgentOptions(usageHooks);
  const workspaceOptions = usageRankingWorkspaceOptions(workspaces);
  const updateFilters = (patch) => onFilters({ ...filters, ...patch });

  return (
    <section className="dashboardFrame rankingsFrame" aria-label="Rankings">
      {error ? <div className="notice">{error}</div> : null}
      <PageTitleRow
        title="Rankings"
        count={rows.length}
        actions={(
          <button className="button secondary" disabled={loading} type="button" onClick={onRefresh}>
            <RefreshCw aria-hidden="true" />
            {loading ? 'Loading...' : 'Refresh'}
          </button>
        )}
      />

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
    </section>
  );
}
