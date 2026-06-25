import React, { useEffect, useState } from 'react';
import {
  Grid3X3,
  Import as ImportIcon,
  List,
  PackagePlus,
  RefreshCw,
  Search,
  Star,
  X
} from 'lucide-react';
import { dashboardTabItems } from '../dashboardFilters.js';
import { labelize } from '../skills.js';
import {
  formatStatusNoticeCountdown,
  statusNoticeAutoCloseSeconds
} from '../skillStatusRefresh.js';
import { AgentIconStack, Badge, Icon, PageTitleRow } from './common.jsx';

export function Dashboard({
  activeTag,
  counts,
  error,
  favoritesOnly,
  filter,
  filterOptions,
  filtered,
  isFirstUse,
  lastStatusCheckedLabel,
  notice,
  query,
  status,
  viewMode,
  onFavoritesOnly,
  onFilter,
  onInstall,
  onOpenSkill,
  onQuery,
  onRefresh,
  onRefreshStatuses,
  onTagFilter,
  onToggleFavorite,
  onViewMode,
  onDismissNotice
}) {
  const isChecking = status === 'checking';
  const tabs = dashboardTabItems(counts);

  return (
    <>
      {error ? <div className="notice">{error}</div> : null}
      {isFirstUse && notice ? <div className="notice success">{notice}</div> : null}

      {isFirstUse ? (
        <FirstUseDashboard status={status} onInstall={onInstall} onScan={onRefresh} />
      ) : (
        <section className="dashboardFrame" aria-label="Skills dashboard">
          <PageTitleRow title="Skills" count={filtered.length} />

          <div className="dashboardControlRow">
            <label className="searchField dashboardSearch" aria-label="Search skills">
              <Search aria-hidden="true" />
              <input
                value={query}
                onChange={(event) => onQuery(event.target.value)}
                name="skill-search"
                placeholder="Search skills in SkillBox..."
                type="search"
              />
            </label>

            <div className="dashboardTypeTabs" role="tablist" aria-label="Skill type">
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

            <DashboardActionGroup
              isChecking={isChecking}
              onInstall={onInstall}
              onRefresh={onRefresh}
              onRefreshStatuses={onRefreshStatuses}
            />

            <div className="viewSwitch" role="group" aria-label="Dashboard view">
              <button
                aria-label="Show card view"
                aria-pressed={viewMode === 'grid'}
                className={viewMode === 'grid' ? 'active' : ''}
                type="button"
                onClick={() => onViewMode('grid')}
              >
                <Grid3X3 aria-hidden="true" />
              </button>
              <button
                aria-label="Show list view"
                aria-pressed={viewMode === 'list'}
                className={viewMode === 'list' ? 'active' : ''}
                type="button"
                onClick={() => onViewMode('list')}
              >
                <List aria-hidden="true" />
              </button>
            </div>
          </div>

          {notice ? (
            <DashboardStatusNotice message={notice} onDismiss={onDismissNotice} />
          ) : null}

          <div className="dashboardFilterRow" aria-label="Dashboard filters">
            <DashboardChipGroup
              active={activeTag}
              allLabel="All tags"
              label="Tags"
              options={filterOptions.tags}
              onSelect={onTagFilter}
            />
            <button
              aria-pressed={favoritesOnly}
              className={favoritesOnly ? 'favoriteFilterButton active' : 'favoriteFilterButton'}
              type="button"
              onClick={() => onFavoritesOnly(!favoritesOnly)}
            >
              <Star aria-hidden="true" />
              Favorites
            </button>
          </div>

          {viewMode === 'grid' ? (
            <div className="skillCardGrid" aria-label="Skill cards">
              {filtered.map((skill) => (
                <SkillCard
                  key={`${skill.sourceRoot}-${skill.name}`}
                  skill={skill}
                  onOpen={onOpenSkill}
                  onToggleFavorite={onToggleFavorite}
                />
              ))}
            </div>
          ) : (
            <div className="skillsTable dashboardList" role="table" aria-label="All skills">
              <div className="tableHeader" role="row">
                <span>Name</span>
                <span>Type</span>
                <span>Status</span>
                <span>Checked</span>
              </div>

              {filtered.map((skill) => (
                <button
                  className="tableRow"
                  key={`${skill.sourceRoot}-${skill.name}`}
                  type="button"
                  onClick={() => onOpenSkill(skill)}
                >
                  <span className="skillNameCell">
                    <strong>{skill.name}</strong>
                    <small>{skill.description || 'No description in SKILL.md'}</small>
                    <span className="tableTagLine">{skill.displayTags.join(', ')}</span>
                  </span>
                  <Badge tone={skill.type === 'user' ? 'green' : 'blue'}>{labelize(skill.type)}</Badge>
                  <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
                  <span className="checkedText">{lastStatusCheckedLabel}</span>
                </button>
              ))}
            </div>
          )}

          {filtered.length === 0 ? (
            <div className="emptyState dashboardEmptyState">
              <strong>No skills found</strong>
              <span>Try another filter or run a fresh scan.</span>
            </div>
          ) : null}
        </section>
      )}
    </>
  );
}

function DashboardActionGroup({ isChecking, onInstall, onRefresh, onRefreshStatuses }) {
  const [previewAction, setPreviewAction] = useState(null);
  const previewIndex = { refresh: 0, import: 1, install: 2 }[previewAction] || 0;

  const actions = [
    {
      id: 'refresh',
      icon: RefreshCw,
      label: isChecking ? 'Refreshing' : 'Refresh',
      loading: isChecking,
      disabled: isChecking,
      onClick: onRefreshStatuses
    },
    {
      id: 'import',
      icon: ImportIcon,
      label: 'Import',
      onClick: onRefresh
    },
    {
      id: 'install',
      icon: PackagePlus,
      label: 'Install',
      onClick: onInstall
    }
  ];

  return (
    <div
      className={previewAction ? 'dashboardActionGroup previewing' : 'dashboardActionGroup'}
      aria-label="Skill actions"
      style={{ '--dashboard-action-index': previewIndex }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          setPreviewAction(null);
        }
      }}
      onMouseLeave={() => setPreviewAction(null)}
    >
      <span className="dashboardActionIndicator" aria-hidden="true" />
      {actions.map((action) => {
        const Icon = action.icon;
        const actionClassName = action.loading
          ? 'dashboardActionButton loading'
          : previewAction === action.id
            ? 'dashboardActionButton preview'
            : 'dashboardActionButton';
        return (
          <button
            aria-busy={action.loading ? 'true' : undefined}
            className={actionClassName}
            disabled={action.disabled}
            key={action.id}
            type="button"
            onFocus={() => setPreviewAction(action.id)}
            onMouseEnter={() => setPreviewAction(action.id)}
            onClick={() => {
              action.onClick();
              setPreviewAction(null);
            }}
          >
            <Icon aria-hidden="true" />
            {action.label}
          </button>
        );
      })}
    </div>
  );
}

function DashboardChipGroup({ active, allLabel, label, options, onSelect }) {
  return (
    <div className="dashboardChipGroup">
      <span>{label}</span>
      <div>
        <button
          className={active === 'all' ? 'active' : ''}
          type="button"
          onClick={() => onSelect('all')}
        >
          {allLabel}
        </button>
        {options.map((option) => (
          <button
            className={active === option ? 'active' : ''}
            key={option}
            type="button"
            onClick={() => onSelect(option)}
          >
            {option}
          </button>
        ))}
      </div>
    </div>
  );
}

function SkillCard({ skill, onOpen, onToggleFavorite }) {
  return (
    <article className={skill.isFavorite ? 'skillCard favorite' : 'skillCard'}>
      <button className="skillCardHitArea" type="button" onClick={() => onOpen(skill)}>
        <span className="skillCardTitleRow">
          <span className="skillCardTitleText">
            <strong>{skill.name}</strong>
            <span className="skillCardUsage">{skill.usageCount || 0} calls</span>
          </span>
        </span>
        <span className="skillCardDescription">
          {skill.description || 'No description in SKILL.md'}
        </span>
        <span className="skillCardTags">
          {skill.displayTags.map((tag) => (
            <span className="tagPill" key={tag}>
              {tag}
            </span>
          ))}
        </span>
        <span className="skillCardMeta">
          <Badge tone={skill.type === 'user' ? 'green' : 'blue'}>{labelize(skill.type)}</Badge>
          <AgentIconStack agents={skill.installedAgents} />
        </span>
      </button>
      <span className="skillCardHeaderActions">
        <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
        <button
          aria-label={skill.isFavorite ? `Remove ${skill.name} from favorites` : `Add ${skill.name} to favorites`}
          aria-pressed={skill.isFavorite}
          className={skill.isFavorite ? 'skillFavoriteButton active' : 'skillFavoriteButton'}
          type="button"
          onClick={() => onToggleFavorite(skill.name)}
        >
          <Star aria-hidden="true" />
        </button>
      </span>
    </article>
  );
}

export function DashboardStatusNotice({ message, onDismiss }) {
  const [remainingSeconds, setRemainingSeconds] = useState(statusNoticeAutoCloseSeconds);

  useEffect(() => {
    const startedAt = Date.now();

    setRemainingSeconds(statusNoticeAutoCloseSeconds);

    const intervalId = window.setInterval(() => {
      const elapsedSeconds = Math.floor((Date.now() - startedAt) / 1000);
      setRemainingSeconds(Math.max(statusNoticeAutoCloseSeconds - elapsedSeconds, 0));
    }, 250);
    const timeoutId = window.setTimeout(onDismiss, statusNoticeAutoCloseSeconds * 1000);

    return () => {
      window.clearInterval(intervalId);
      window.clearTimeout(timeoutId);
    };
  }, [message]);

  return (
    <div className="panelNotice notice success dashboardStatusNotice" role="status">
      <span className="dashboardStatusNoticeMessage">{message}</span>
      <span className="dashboardStatusNoticeCountdown">
        {formatStatusNoticeCountdown(remainingSeconds)}
      </span>
      <button
        className="noticeDismissButton"
        type="button"
        aria-label="Dismiss status notice"
        onClick={onDismiss}
      >
        <X aria-hidden="true" size={14} />
      </button>
    </div>
  );
}

function FirstUseDashboard({ status, onInstall, onScan }) {
  return (
    <section className="firstUseGrid firstUseOnly">
      <div className="panel firstUsePanel">
        <div className="emptyGlyph">
          <Icon name="dashboard" />
        </div>
        <div>
          <p className="eyebrow">First import</p>
          <h2>No skills imported yet</h2>
          <p>
            SkillBox will scan local runtime folders, show the candidates first, and only import the
            skills you confirm.
          </p>
        </div>
        <div className="firstUseActions">
          <button className="button primary" type="button" onClick={onScan}>
            {status === 'scanning' ? 'Scanning...' : 'Scan local skills'}
          </button>
          <button className="button secondary" type="button" onClick={onInstall}>
            Import from remote
          </button>
        </div>
      </div>
    </section>
  );
}
