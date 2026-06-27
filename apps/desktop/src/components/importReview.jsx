import React, { useState } from 'react';
import { Search } from 'lucide-react';
import codexAppIcon from '../assets/codex-app-icon.png';
import codexCliIcon from '../assets/codex-cli-icon.png';
import {
  filterImportCandidatesByQuery,
  filterWorkspaceSkillCandidates,
  visibleImportCandidates,
  workspaceSkillTabs
} from '../importCandidates.js';
import {
  candidateRowClass,
  candidateSource,
  candidateStatusNote,
  isImportableCandidate
} from '../importFlow.js';
import { closeOnBackdropClick } from '../modalEvents.js';
import { compactPath } from '../skills.js';
import { Badge } from './common.jsx';

export function RemoteImportDialog({ error, mode, status, value, onClose, onModeChange, onSubmit, onValueChange }) {
  const isMarkdown = mode === 'markdown';

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="remoteImportDialog" role="dialog" aria-modal="true" aria-labelledby="remote-import-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-import-title">Import skill</h2>
            <p>Provide a skill URL or a local Markdown file to review before importing.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close remote import" onClick={onClose}>
            x
          </button>
        </div>

        <form className="remoteImportForm" onSubmit={onSubmit}>
          <div className="remoteImportModes" role="group" aria-label="Import source type">
            <button
              className={mode === 'url' ? 'active' : ''}
              type="button"
              onClick={() => onModeChange('url')}
            >
              Skill URL
            </button>
            <button
              className={isMarkdown ? 'active' : ''}
              type="button"
              onClick={() => onModeChange('markdown')}
            >
              Markdown file
            </button>
          </div>

          <label className="remoteImportField">
            <span>{isMarkdown ? 'Markdown file path' : 'Skill URL'}</span>
            <input
              autoFocus
              placeholder={
                isMarkdown
                  ? '~/Downloads/SKILL.md'
                  : 'https://github.com/owner/repo/tree/main/path/to/skill'
              }
              type={isMarkdown ? 'text' : 'url'}
              value={value}
              onChange={(event) => onValueChange(event.target.value)}
            />
          </label>

          <p className="remoteImportHint">
            {isMarkdown
              ? 'Use a local .md file path. SkillBox will turn it into a reviewable import candidate.'
              : 'Use a GitHub tree, blob, raw, or API URL that points to a skill directory or SKILL.md.'}
          </p>
          {error ? <div className="formError">{error}</div> : null}

          <div className="remoteImportFooter">
            <button className="button secondary" type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={status === 'importing'} type="submit">
              Review import
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

export function LocalImportConfirmationDialog({
  candidates,
  dontShowAgain,
  status,
  onClose,
  onConfirm,
  onDontShowAgainChange
}) {
  const shownCandidates = candidates.slice(0, 3);
  const remainingCount = Math.max(candidates.length - shownCandidates.length, 0);

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="localImportDialog" role="dialog" aria-modal="true" aria-labelledby="local-import-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="local-import-title">Confirm local import</h2>
            <p>SkillBox will move the selected skill folders into the managed store.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close local import confirmation" onClick={onClose}>
            x
          </button>
        </div>

        <div className="localImportBody">
          <div className="localImportImpact">
            <strong>{candidates.length} selected</strong>
            <p>
              The original folders will be replaced with symlinks to the managed copies, and the
              moved folders will be kept under the SkillBox import backups.
            </p>
          </div>

          <ul className="localImportPaths" aria-label="Selected local skill paths">
            {shownCandidates.map((candidate) => (
              <li key={candidate.sourcePath}>
                <span>{candidate.name}</span>
                <code>{compactPath(candidate.sourcePath)}</code>
              </li>
            ))}
            {remainingCount > 0 ? <li className="muted">+{remainingCount} more</li> : null}
          </ul>

          <label className="localImportPreference">
            <input
              checked={dontShowAgain}
              type="checkbox"
              onChange={(event) => onDontShowAgainChange(event.target.checked)}
            />
            <span>Don't show this again</span>
          </label>
        </div>

        <div className="localImportFooter">
          <button className="button secondary" disabled={status === 'importing'} type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={status === 'importing'} type="button" onClick={onConfirm}>
            {status === 'importing' ? 'Importing...' : 'Confirm import'}
          </button>
        </div>
      </section>
    </div>
  );
}

export function ImportReview({
  candidates,
  errors = [],
  onClose,
  onImport,
  onToggleAll,
  onToggleSelected,
  onTypeChange,
  status,
  subtitle = 'Confirm each skill type before SkillBox copies it into the managed store.',
  title = 'Import Review'
}) {
  const visibleCandidates = visibleImportCandidates(candidates);
  const selectableCount = visibleCandidates.filter(isImportableCandidate).length;
  const selectedCount = visibleCandidates.filter(
    (candidate) => candidate.isSelected && isImportableCandidate(candidate)
  ).length;
  const isAllSelected = selectableCount > 0 && selectedCount === selectableCount;

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="importSheet" role="dialog" aria-modal="true" aria-labelledby="import-review-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="import-review-title">{title}</h2>
            <p>{subtitle}</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close import review" onClick={onClose}>
            x
          </button>
        </div>

        <div className="candidateList">
          {errors.length > 0 ? (
            <div className="workspaceSkillError">
              {errors.length} scan {errors.length === 1 ? 'issue' : 'issues'} found.
            </div>
          ) : null}
          {visibleCandidates.length === 0 && errors.length === 0 ? (
            <div className="emptyState dashboardEmptyState workspaceSkillEmptyState">
              <strong>No skills found</strong>
              <span>This workspace has no importable SKILL.md directories yet.</span>
            </div>
          ) : null}
          {visibleCandidates.length > 0 ? (
            <CandidateReviewList
              candidates={visibleCandidates}
              onToggleSelected={onToggleSelected}
              onTypeChange={onTypeChange}
            />
          ) : null}
        </div>

        <div className="importSheetFooter">
          <div className="importSelectionSummary">
            <button
              className="selectAllButton"
              disabled={selectableCount === 0 || status === 'importing'}
              type="button"
              onClick={onToggleAll}
            >
              {isAllSelected ? 'Unselect all' : 'Select all'}
            </button>
            <span>{selectedCount} selected</span>
          </div>
          <div className="headerActions">
            <button className="button secondary" type="button" onClick={onClose}>
              Cancel
            </button>
            <button
              className="button primary"
              disabled={status === 'importing' || selectedCount === 0}
              type="button"
              onClick={onImport}
            >
              {status === 'importing' ? 'Importing...' : 'Import selected'}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function CandidateReviewList({ candidates, onToggleSelected, onTypeChange }) {
  const [activeTab, setActiveTab] = useState('all');
  const [searchQuery, setSearchQuery] = useState('');
  const searchedCandidates = filterImportCandidatesByQuery(candidates, searchQuery);
  const tabs = workspaceSkillTabs(searchedCandidates);
  const filteredCandidates = filterWorkspaceSkillCandidates(searchedCandidates, activeTab);

  return (
    <>
      <div className="candidateReviewToolbar">
        <label className="searchField candidateSearchField" aria-label="Search review skills">
          <Search aria-hidden="true" />
          <input
            autoCapitalize="none"
            autoComplete="off"
            autoCorrect="off"
            inputMode="search"
            name="import-review-search"
            placeholder="Search review skills..."
            role="searchbox"
            spellCheck={false}
            type="text"
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
          />
        </label>
        <WorkspaceSkillTabs
          activeTab={activeTab}
          tabs={tabs}
          onTabChange={setActiveTab}
        />
      </div>
      {filteredCandidates.length > 0 ? (
        filteredCandidates.map((candidate) => (
          <CandidateRow
            candidate={candidate}
            key={candidate.sourcePath}
            onToggleSelected={onToggleSelected}
            onTypeChange={onTypeChange}
          />
        ))
      ) : (
        <div className="emptyState dashboardEmptyState workspaceSkillEmptyState">
          <strong>No skills in this view</strong>
          <span>{searchQuery ? 'Try another search or switch tabs.' : 'Switch tabs to review the rest.'}</span>
        </div>
      )}
    </>
  );
}

function WorkspaceSkillTabs({ activeTab, tabs, onTabChange }) {
  return (
    <div className="workspaceSkillTabs" role="tablist" aria-label="Workspace skill view">
      {tabs.map((tab) => (
        <button
          aria-selected={activeTab === tab.id}
          className={activeTab === tab.id ? 'active' : ''}
          key={tab.id}
          role="tab"
          type="button"
          onClick={() => onTabChange(tab.id)}
        >
          <span>{tab.label}</span>
          <strong>{tab.count}</strong>
        </button>
      ))}
    </div>
  );
}

function CandidateRow({ candidate, onToggleSelected, onTypeChange }) {
  return (
    <div className={candidateRowClass(candidate)}>
      <label className="candidateCheck">
        <input
          checked={candidate.isSelected}
          disabled={!isImportableCandidate(candidate)}
          type="checkbox"
          onChange={() => onToggleSelected(candidate)}
        />
        <span />
      </label>

      <div className="candidateMain">
        <div className="candidateTitle">
          <strong>{candidate.name}</strong>
          <SourceIcon candidate={candidate} />
          <Badge tone={candidate.skillType === 'user' ? 'green' : 'blue'}>
            {candidate.skillType === 'user' ? 'User skill' : 'Remote skill'}
          </Badge>
          {candidate.importStatus === 'system' ? <Badge tone="slate">System</Badge> : null}
          {candidate.importStatus === 'imported' ? <Badge tone="slate">Imported</Badge> : null}
          {candidate.conflict ? <Badge tone="red">Conflict</Badge> : null}
        </div>
        <small>{candidate.description || 'No description in SKILL.md'}</small>
        <span className="candidatePath">
          <span>Path</span>
          <code>{compactPath(candidate.sourcePath)}</code>
        </span>
        {candidate.isSymlink ? (
          <span className="candidateSymlinkSource">
            <span>Symlink source</span>
            <code>{compactPath(candidate.symlinkTargetPath || candidate.realPath || '')}</code>
          </span>
        ) : null}
        <span className="candidateUsage">Calls {candidate.usageCount || 0}</span>
        {candidateStatusNote(candidate) ? <p>{candidateStatusNote(candidate)}</p> : null}
      </div>

      <div className="candidateTypeSwitch" role="group" aria-label={`${candidate.name} type`}>
        <button
          className={candidate.skillType === 'user' ? 'active' : ''}
          disabled={!isImportableCandidate(candidate)}
          type="button"
          onClick={() => onTypeChange(candidate, 'user')}
        >
          User
        </button>
        <button
          className={candidate.skillType === 'remote' ? 'active' : ''}
          disabled={!isImportableCandidate(candidate)}
          type="button"
          onClick={() => onTypeChange(candidate, 'remote')}
        >
          Remote
        </button>
      </div>
    </div>
  );
}

function SourceIcon({ candidate }) {
  const source = candidateSource(candidate);
  if (!source) {
    return null;
  }

  const iconSource = source.kind === 'agent' ? codexCliIcon : codexAppIcon;

  return (
    <span className={`sourceIcon ${source.kind}`} title={source.label} aria-label={source.label}>
      <img src={iconSource} alt="" aria-hidden="true" />
    </span>
  );
}
