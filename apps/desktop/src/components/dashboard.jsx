import React, { useEffect, useRef, useState } from 'react';
import {
  Cloud,
  Grid3X3,
  GitBranch,
  Import as ImportIcon,
  List,
  PackagePlus,
  RefreshCw,
  ScanSearch,
  Search,
  ShieldCheck,
  Star,
  UserRound,
  X
} from 'lucide-react';
import { dashboardTabItems } from '../dashboardFilters.js';
import { labelize } from '../skills.js';
import {
  formatStatusNoticeCountdown,
  statusNoticeAutoCloseSeconds
} from '../skillStatusRefresh.js';
import { AgentIconStack, Badge, PageFrame, PageTitleRow } from './common.jsx';

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
  onClearFilters,
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
  const searchInputRef = useRef(null);
  const hasActiveFilters = Boolean(
    query.trim() || filter !== 'all' || activeTag !== 'all' || favoritesOnly
  );

  useEffect(() => {
    function focusDashboardSearch(event) {
      if (
        (event.metaKey || event.ctrlKey) &&
        event.key.toLowerCase() === 'f' &&
        !document.querySelector('[role="dialog"]')
      ) {
        event.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    }

    window.addEventListener('keydown', focusDashboardSearch);
    return () => window.removeEventListener('keydown', focusDashboardSearch);
  }, []);

  return (
    <>
      {error ? <div className="notice">{error}</div> : null}
      {isFirstUse && notice ? <div className="notice success">{notice}</div> : null}

      {isFirstUse ? (
        <FirstUseDashboard status={status} onInstall={onInstall} onScan={onRefresh} />
      ) : (
        <PageFrame ariaLabel="Skills dashboard">
          <PageTitleRow
            title="Skills"
            count={filtered.length}
            actions={(
              <div className="dashboardPageActions">
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
                    title="Card view"
                    type="button"
                    onClick={() => onViewMode('grid')}
                  >
                    <Grid3X3 aria-hidden="true" />
                  </button>
                  <button
                    aria-label="Show list view"
                    aria-pressed={viewMode === 'list'}
                    className={viewMode === 'list' ? 'active' : ''}
                    title="List view"
                    type="button"
                    onClick={() => onViewMode('list')}
                  >
                    <List aria-hidden="true" />
                  </button>
                </div>
              </div>
            )}
          />

          <div className="dashboardFilterBar" aria-label="Dashboard filters">
            <div className="dashboardFilterPrimary">
              <label className="searchField dashboardSearch" aria-label="Search skills">
                <Search aria-hidden="true" />
                <input
                  ref={searchInputRef}
                  value={query}
                  onChange={(event) => onQuery(event.target.value)}
                  name="skill-search"
                  placeholder="Search skills in SkillBox..."
                  type="search"
                />
                <span className="searchShortcutHint" aria-hidden="true">⌘F</span>
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

            <DashboardChipGroup
              active={activeTag}
              allLabel="All tags"
              label="Tags"
              options={filterOptions.tags}
              onSelect={onTagFilter}
            />
          </div>

          {notice ? (
            <DashboardStatusNotice message={notice} onDismiss={onDismissNotice} />
          ) : null}

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
                  <SkillTypeBadge type={skill.type} />
                  <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
                  <span className="checkedText">{lastStatusCheckedLabel}</span>
                </button>
              ))}
            </div>
          )}

          {filtered.length === 0 ? (
            <div className="emptyState dashboardEmptyState">
              <strong>No skills found</strong>
              <span>{hasActiveFilters ? 'No skills match the current filters.' : 'Run a fresh scan to find skills.'}</span>
              {hasActiveFilters ? (
                <button className="button secondary" type="button" onClick={onClearFilters}>
                  Clear filters
                </button>
              ) : null}
            </div>
          ) : null}
        </PageFrame>
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
  const cardClassName = [
    'skillCard',
    `status-${skill.statusTone}`,
    skill.isFavorite ? 'favorite' : ''
  ].filter(Boolean).join(' ');

  return (
    <article className={cardClassName}>
      <button className="skillCardHitArea" type="button" onClick={() => onOpen(skill)}>
        <span className="skillCardTitleRow">
          <span className="skillCardTitleText">
            <strong>{skill.name}</strong>
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
          <span className="skillCardMetaDetails">
            <SkillTypeBadge type={skill.type} />
            <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
            {skill.usageCount > 0 ? (
              <span className="skillCardUsage">
                {skill.usageCount} locally observed calls
              </span>
            ) : null}
          </span>
          <AgentIconStack agents={skill.installedAgents} />
        </span>
      </button>
      <button
        aria-label={skill.isFavorite ? `Remove ${skill.name} from favorites` : `Add ${skill.name} to favorites`}
        aria-pressed={skill.isFavorite}
        className={skill.isFavorite ? 'skillFavoriteButton active' : 'skillFavoriteButton'}
        type="button"
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation();
          void onToggleFavorite(skill.name);
        }}
      >
        <Star aria-hidden="true" />
      </button>
    </article>
  );
}

function SkillTypeBadge({ type }) {
  const TypeIcon = type === 'user' ? UserRound : Cloud;

  return (
    <Badge tone="slate">
      <span className="skillTypeBadge">
        <TypeIcon aria-hidden="true" />
        {labelize(type)}
      </span>
    </Badge>
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
      <div className="dashboardStatusNoticeActions">
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
    </div>
  );
}

function FirstUseDashboard({ status, onInstall, onScan }) {
  return (
    <section className="firstUseGrid firstUseOnly" aria-labelledby="first-use-title">
      <div className="panel firstUsePanel">
        <div className="firstUseHeader">
          <div>
            <p className="eyebrow">First run setup</p>
            <h2 id="first-use-title">Set up SkillBox safely</h2>
            <p>
              Start with a scan. SkillBox does not change runtime folders until you review and
              confirm an action.
            </p>
          </div>
          <div className="firstUseWorkflow" aria-label="Safe setup flow">
            <span className="firstUseFlowItem">
              <ScanSearch aria-hidden="true" />
              Read-only scan
            </span>
            <span className="firstUseFlowItem">
              <ShieldCheck aria-hidden="true" />
              Review gate
            </span>
            <span className="firstUseFlowItem">
              <GitBranch aria-hidden="true" />
              Linked deploy
            </span>
          </div>
        </div>

        <div className="firstUseChecklist" aria-label="Safe setup checklist">
          <div className="firstUseStep">
            <span className="firstUseStepIcon" aria-hidden="true">
              <ScanSearch />
            </span>
            <div>
              <strong>Scan workspaces</strong>
              <p>The scan is read-only and checks common Codex, Claude, and project skill roots.</p>
            </div>
          </div>
          <div className="firstUseStep">
            <span className="firstUseStepIcon" aria-hidden="true">
              <ShieldCheck />
            </span>
            <div>
              <strong>Review imports</strong>
              <p>
                Candidates are reviewed and classified before SkillBox copies anything into{' '}
                <code>~/.skillbox</code>.
              </p>
            </div>
          </div>
          <div className="firstUseStep">
            <span className="firstUseStepIcon" aria-hidden="true">
              <GitBranch />
            </span>
            <div>
              <strong>Deploy intentionally</strong>
              <p>Runtime folders are linked only when you choose a workspace target.</p>
            </div>
          </div>
        </div>

        <div className="firstUseSafetyStrip" aria-label="Safety boundaries">
          <span>Read-only scan</span>
          <span>Review before copy</span>
          <span>No silent overwrite</span>
        </div>

        <div className="firstUseActions">
          <button className="button primary" type="button" onClick={onScan}>
            {status === 'scanning' ? 'Scanning...' : 'Scan local skills'}
          </button>
          <button className="button secondary" type="button" onClick={onInstall}>
            Install from GitHub
          </button>
        </div>
      </div>
    </section>
  );
}
