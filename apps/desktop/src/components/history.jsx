import React from 'react';
import { RefreshCw } from 'lucide-react';
import {
  groupHistoryEntriesByDay,
  historyRowSubtitle,
  operationStatusTone
} from '../historyEntries.js';
import { formatOperationTimestamp } from '../remoteSkills.js';
import { compactPath, numberOrZero } from '../skills.js';
import { Badge, PageTitleRow } from './common.jsx';

export function HistoryPage({ error, filter, history, status, onFilter, onRefresh }) {
  const entries = history.entries || [];
  const tabs = [
    {
      id: 'all',
      label: 'All',
      count: numberOrZero(history.skillUsageCount) + numberOrZero(history.operationCount)
    },
    { id: 'skill_usage', label: 'Skill calls', count: numberOrZero(history.skillUsageCount) },
    { id: 'operation', label: 'Operations', count: numberOrZero(history.operationCount) }
  ];
  const filteredEntries =
    filter === 'all' ? entries : entries.filter((entry) => entry.kind === filter);
  const groupedEntries = groupHistoryEntriesByDay(filteredEntries);
  const isLoading = status === 'loading_history';

  return (
    <section className="dashboardFrame historyFrame" aria-label="History">
      {error ? <div className="notice">{error}</div> : null}
      <PageTitleRow title="History" count={filteredEntries.length} />

      <div className="dashboardControlRow historyControlRow">
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
        <button className="button secondary" disabled={isLoading} type="button" onClick={onRefresh}>
          <RefreshCw aria-hidden="true" />
          {isLoading ? 'Loading...' : 'Refresh'}
        </button>
      </div>

      {filteredEntries.length > 0 ? (
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
