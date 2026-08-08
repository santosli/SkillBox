import React, { useState } from 'react';
import { ChevronDown, MapPin, Search } from 'lucide-react';
import codexAppIcon from '../assets/codex-app-icon.png';
import codexCliIcon from '../assets/codex-cli-icon.png';
import {
  canClassifyImportCandidateGroup,
  filterImportCandidateGroups,
  filterImportCandidateGroupsByQuery,
  filterImportCollectionsByQuery,
  importCollectionGroupIds,
  importCandidateGroupLocationCount,
  importCandidateGroupStatus,
  importCandidateGroupTabs,
  isSelectableImportCandidateGroup,
  selectedImportCandidate,
  selectedImportCandidateVariant
} from '../importCandidates.js';
import {
  candidateImportSourcePaths,
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
              : 'Use a GitHub repository, tree, blob, raw, or API URL. Standalone repositories with a root SKILL.md and skill directories are supported.'}
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
  status,
  onClose,
  onConfirm,
  onTypeChange
}) {
  const shownCandidates = candidates.slice(0, 3);
  const remainingCount = Math.max(candidates.length - shownCandidates.length, 0);
  const untouchedCopyCount = candidates.reduce(
    (count, candidate) => count + Math.max(candidateImportSourcePaths(candidate).length - 1, 0),
    0
  );

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
            <p>Choose User or Remote, then SkillBox will move the selected skill folders into the managed store.</p>
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
              {untouchedCopyCount > 0
                ? ` ${untouchedCopyCount} identical ${untouchedCopyCount === 1 ? 'copy' : 'copies'} shown in review will remain unchanged.`
                : ''}
            </p>
          </div>

          <ul className="localImportPaths" aria-label="Selected local skill paths">
            {shownCandidates.map((candidate) => {
              const skillType = candidate.skillType === 'remote' ? 'remote' : 'user';

              return (
                <li key={candidate.sourcePath}>
                  <div className="localImportPathMeta">
                    <span>{candidate.name}</span>
                    <code>{compactPath(candidate.sourcePath)}</code>
                  </div>
                  <div className="candidateTypeSwitch" role="group" aria-label={`${candidate.name} type`}>
                    <button
                      className={skillType === 'user' ? 'active' : ''}
                      disabled={status === 'importing'}
                      type="button"
                      onClick={() => onTypeChange(candidate, 'user')}
                    >
                      User
                    </button>
                    <button
                      className={skillType === 'remote' ? 'active' : ''}
                      disabled={status === 'importing'}
                      type="button"
                      onClick={() => onTypeChange(candidate, 'remote')}
                    >
                      Remote
                    </button>
                  </div>
                </li>
              );
            })}
            {remainingCount > 0 ? <li className="muted">+{remainingCount} more</li> : null}
          </ul>

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
  groups,
  collections = [],
  errors = [],
  onClose,
  onImport,
  onToggleAll,
  onToggleSelected,
  onSelectVariant,
  onTypeChange,
  status,
  subtitle = 'Confirm each skill type before SkillBox copies it into the managed store.',
  title = 'Import Review'
}) {
  const selectableCount = groups.filter(isSelectableImportCandidateGroup).length;
  const selectedCount = groups.filter(
    (group) => group.isSelected && isSelectableImportCandidateGroup(group)
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
          {groups.length === 0 && errors.length === 0 ? (
            <div className="emptyState dashboardEmptyState workspaceSkillEmptyState">
              <strong>No skills found</strong>
              <span>This workspace has no importable SKILL.md directories yet.</span>
            </div>
          ) : null}
          {groups.length > 0 ? (
            <CandidateReviewList
              collections={collections}
              groups={groups}
              onSelectVariant={onSelectVariant}
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

function CandidateReviewList({ collections, groups, onSelectVariant, onToggleSelected, onTypeChange }) {
  const [activeTab, setActiveTab] = useState('all');
  const [searchQuery, setSearchQuery] = useState('');
  const searchedGroups = filterImportCandidateGroupsByQuery(groups, searchQuery);
  const searchedCollections = filterImportCollectionsByQuery(collections, searchQuery);
  const tabs = importCandidateGroupTabs(searchedGroups);
  const filteredGroups = filterImportCandidateGroups(searchedGroups, activeTab);
  const collectionGroupIds = importCollectionGroupIds(collections);
  const visibleCollections = searchedCollections
    .map((collection) => ({
      ...collection,
      children: collection.children.filter((child) => {
        const group = groups.find((candidateGroup) => candidateGroup.id === child.groupId);
        return group && filterImportCandidateGroups([group], activeTab).length > 0;
      })
    }))
    .filter((collection) => collection.children.length > 0);
  const standaloneGroups = filteredGroups.filter((group) => !collectionGroupIds.has(group.id));

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
      {visibleCollections.map((collection) => (
        <CollectionReviewCard
          collection={collection}
          groups={groups}
          key={collection.id}
          onSelectVariant={onSelectVariant}
          onToggleSelected={onToggleSelected}
          onTypeChange={onTypeChange}
        />
      ))}
      {standaloneGroups.length > 0 ? (
        standaloneGroups.map((group) => (
          <CandidateGroupCard
            group={group}
            key={group.id}
            onSelectVariant={onSelectVariant}
            onToggleSelected={onToggleSelected}
            onTypeChange={onTypeChange}
          />
        ))
      ) : visibleCollections.length === 0 ? (
        <div className="emptyState dashboardEmptyState workspaceSkillEmptyState">
          <strong>No skills in this view</strong>
          <span>{searchQuery ? 'Try another search or switch tabs.' : 'Switch tabs to review the rest.'}</span>
        </div>
      ) : null}
    </>
  );
}

function CollectionReviewCard({
  collection,
  groups,
  onSelectVariant,
  onToggleSelected,
  onTypeChange
}) {
  const [expanded, setExpanded] = useState(false);
  const disclosureId = `${collection.id}-children`;
  const selectedCount = collection.children.filter((child) => {
    const group = groups.find((candidateGroup) => candidateGroup.id === child.groupId);
    return group?.isSelected && group.selectedVariantId === child.variantId;
  }).length;
  const shortSha = collection.reviewedHeadSha ? collection.reviewedHeadSha.slice(0, 8) : 'uncommitted';

  return (
    <section className="collectionReviewCard" aria-label={`${collection.displayName} skill collection`}>
      <div className="collectionReviewHeader">
        <div className="collectionReviewIdentity">
          <div className="candidateTitle">
            <strong>{collection.displayName}</strong>
            <Badge tone="slate">Collection</Badge>
            <Badge tone="slate">{collection.children.length} skills</Badge>
          </div>
          <small>
            {collection.branch || (collection.detached ? 'Detached HEAD' : 'No branch')} · {shortSha}
            {collection.originUrl ? ` · ${collection.originUrl}` : ''}
          </small>
          <code>{compactPath(collection.canonicalWorktreeRoot)}</code>
        </div>
        <span className="collectionReviewSelection">{selectedCount} selected</span>
      </div>
      <button
        aria-controls={disclosureId}
        aria-expanded={expanded}
        className="candidateLocationsDisclosure"
        type="button"
        onClick={() => setExpanded((current) => !current)}
      >
        <MapPin aria-hidden="true" />
        <span>{expanded ? 'Hide collection skills' : 'Review collection skills'}</span>
        <ChevronDown aria-hidden="true" className={expanded ? 'expanded' : ''} />
      </button>
      {expanded ? (
        <div className="collectionReviewChildren" id={disclosureId}>
          {collection.children.map((child) => {
            const group = groups.find((candidateGroup) => candidateGroup.id === child.groupId);
            if (!group) return null;
            const variant = group.variants.find((candidateVariant) => candidateVariant.id === child.variantId);
            const selected = group.isSelected && group.selectedVariantId === child.variantId;
            const canSelect = isSelectableImportCandidateGroup(group);
            return (
              <div className={`collectionChildRow ${selected ? 'selected' : ''}`} key={child.id}>
                <label className="candidateCheck">
                  <input
                    checked={selected}
                    disabled={!canSelect}
                    type="checkbox"
                    aria-label={`Select ${child.name} from ${collection.displayName}`}
                    onChange={() => onToggleSelected(group)}
                  />
                  <span />
                </label>
                <div className="collectionChildMain">
                  <div className="candidateTitle">
                    <strong>{child.name}</strong>
                    {child.importStatus !== 'importable' ? <Badge tone="slate">{child.importStatus}</Badge> : null}
                    {child.conflict ? <Badge tone="red">Conflict</Badge> : null}
                    {child.requiresTypeReview ? <Badge tone="amber">Choose type</Badge> : null}
                  </div>
                  <code>{child.relativePath}</code>
                  <span className="candidateUsage">Calls {group.usageCount || 0}</span>
                  {child.locations.length > 1 ? (
                    <small>{child.locations.length} runtime/source locations resolve to this child.</small>
                  ) : null}
                  {child.unlinkedLocations?.length > 0 ? (
                    <div className="collectionChildUnlinkedLocations">
                      <small>
                        {child.unlinkedLocations.length} standalone {child.unlinkedLocations.length === 1 ? 'copy remains' : 'copies remain'} outside this repository.
                      </small>
                      {child.unlinkedLocations.map((location) => (
                        <code key={location.sourcePath}>{compactPath(location.sourcePath)}</code>
                      ))}
                    </div>
                  ) : null}
                  {group.variants.length > 1 ? (
                    <div className="collectionChildVariants" role="radiogroup" aria-label={`${child.name} source variant`}>
                      {group.variants.map((candidateVariant, index) => (
                        <label key={candidateVariant.id}>
                          <input
                            checked={group.selectedVariantId === candidateVariant.id}
                            disabled={!isImportableCandidate(candidateVariant.candidate)}
                            name={`${group.id}-collection-variant`}
                            type="radio"
                            onChange={() => onSelectVariant(group, candidateVariant)}
                          />
                          <span>Variant {index + 1}</span>
                        </label>
                      ))}
                    </div>
                  ) : null}
                </div>
                <div className="collectionChildType">
                  <span>{variant?.selectedType === 'remote' ? 'Remote' : 'User'}</span>
                  <div className="candidateTypeSwitch" role="radiogroup" aria-label={`${child.name} skill type`}>
                    <button
                      aria-checked={variant?.selectedType === 'user'}
                      className={variant?.selectedType === 'user' ? 'active' : ''}
                      disabled={!canClassifyImportCandidateGroup(group)}
                      role="radio"
                      type="button"
                      onClick={() => onTypeChange(group, 'user')}
                    >
                      User
                    </button>
                    <button
                      aria-checked={variant?.selectedType === 'remote'}
                      className={variant?.selectedType === 'remote' ? 'active' : ''}
                      disabled={!canClassifyImportCandidateGroup(group)}
                      role="radio"
                      type="button"
                      onClick={() => onTypeChange(group, 'remote')}
                    >
                      Remote
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
    </section>
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

function CandidateGroupCard({ group, onSelectVariant, onToggleSelected, onTypeChange }) {
  const [expanded, setExpanded] = useState(false);
  const candidate = selectedImportCandidate(group) || group.variants[0]?.candidate;
  const selectedVariant = selectedImportCandidateVariant(group);
  const needsTypeChoice = Boolean(
    selectedVariant?.requiresTypeReview && !selectedVariant.selectedType
  );
  const status = importCandidateGroupStatus(group);
  const locationCount = importCandidateGroupLocationCount(group);
  const disclosureId = `${group.id}-locations`;
  const typeLabelId = `${group.id}-type-label`;
  const typeHelpId = `${group.id}-type-help`;

  return (
    <div className={candidateRowClass(candidate || {})}>
      <label className="candidateCheck">
        <input
          checked={group.isSelected}
          disabled={!isSelectableImportCandidateGroup(group)}
          type="checkbox"
          aria-label={`Select ${group.name} for import`}
          onChange={() => onToggleSelected(group)}
        />
        <span />
      </label>

      <div className="candidateMain">
        <div className="candidateTitle">
          <strong>{group.name}</strong>
          {candidate ? <SourceIcon candidate={candidate} /> : null}
          <Badge tone="slate">{locationCount} {locationCount === 1 ? 'location' : 'locations'}</Badge>
          {status.imported ? <Badge tone="slate">Imported</Badge> : null}
          {status.system ? <Badge tone="slate">System</Badge> : null}
          {status.conflict ? <Badge tone="red">Conflict</Badge> : null}
          {group.requiresReview && !group.selectedVariantId ? <Badge tone="amber">Needs review</Badge> : null}
          {needsTypeChoice ? <Badge tone="amber">Mixed type suggestions</Badge> : null}
        </div>
        <small>{group.description || 'No description in SKILL.md'}</small>
        <button
          aria-controls={disclosureId}
          aria-expanded={expanded}
          className="candidateLocationsDisclosure"
          type="button"
          onClick={() => setExpanded((current) => !current)}
        >
          <MapPin aria-hidden="true" />
          <span>Found in {locationCount} {locationCount === 1 ? 'location' : 'locations'}</span>
          <ChevronDown aria-hidden="true" className={expanded ? 'expanded' : ''} />
        </button>
        {expanded ? (
          <div className="candidateVariants" id={disclosureId}>
            {group.variants.map((variant, variantIndex) => {
              const importable = isImportableCandidate(variant.candidate);
              const selected = group.selectedVariantId === variant.id;
              return (
                <div className={`candidateVariant ${selected ? 'selected' : ''}`} key={variant.id}>
                  {group.variants.length > 1 ? (
                    <label className="candidateVariantChoice">
                      <input
                        checked={selected}
                        disabled={!importable}
                        name={`${group.id}-variant`}
                        type="radio"
                        value={variant.id}
                        onChange={() => onSelectVariant(group, variant)}
                      />
                      <span>
                        Variant {variantIndex + 1}
                        {selected ? ' · selected for import' : ''}
                      </span>
                    </label>
                  ) : null}
                  <div className="candidateVariantMeta">
                    <Badge tone="slate">
                      {variant.requiresTypeReview
                        ? 'Mixed type suggestions'
                        : variant.suggestedTypes[0] === 'remote'
                          ? 'Remote suggestion'
                          : 'User suggestion'}
                    </Badge>
                    <Badge tone={variant.candidate.conflict ? 'red' : 'slate'}>
                      {variant.candidate.conflict || variant.candidate.importStatus}
                    </Badge>
                  </div>
                  {variant.locations.map((location) => (
                    <div className="candidateLocation" key={location.sourcePath}>
                      <span>{location.isSymlink ? 'Runtime symlink' : 'Skill folder'}</span>
                      <code>{compactPath(location.sourcePath)}</code>
                      {location.isSymlink ? (
                        <small>Source: {compactPath(location.symlinkTargetPath || location.realPath)}</small>
                      ) : null}
                      <small className="candidateLocationSuggestion">
                        {location.suggestedType === 'remote' ? 'Remote suggestion' : 'User suggestion'}
                        {location.suggestionReason ? ` · ${location.suggestionReason}` : ''}
                      </small>
                    </div>
                  ))}
                  {variant.candidate.conflict ? <p>{variant.candidate.conflict}</p> : null}
                </div>
              );
            })}
          </div>
        ) : null}
        <span className="candidateUsage">
          Calls {group.usageCount || 0}
        </span>
        {candidate && candidateStatusNote(candidate) ? <p>{candidateStatusNote(candidate)}</p> : null}
      </div>

      <div className={`candidateTypeAction ${needsTypeChoice ? 'required' : ''}`}>
        <div className="candidateTypeActionHeader">
          <span id={typeLabelId}>Skill type</span>
          {needsTypeChoice ? <strong>Required</strong> : null}
        </div>
        {needsTypeChoice ? (
          <p id={typeHelpId}>Choose where SkillBox should manage this skill.</p>
        ) : null}
        <div
          aria-describedby={needsTypeChoice ? typeHelpId : undefined}
          aria-labelledby={typeLabelId}
          aria-required={needsTypeChoice}
          className="candidateTypeSwitch"
          role="radiogroup"
        >
          <button
            aria-checked={selectedVariant?.selectedType === 'user'}
            className={selectedVariant?.selectedType === 'user' ? 'active' : ''}
            disabled={!canClassifyImportCandidateGroup(group)}
            role="radio"
            type="button"
            onClick={() => onTypeChange(group, 'user')}
          >
            User
          </button>
          <button
            aria-checked={selectedVariant?.selectedType === 'remote'}
            className={selectedVariant?.selectedType === 'remote' ? 'active' : ''}
            disabled={!canClassifyImportCandidateGroup(group)}
            role="radio"
            type="button"
            onClick={() => onTypeChange(group, 'remote')}
          >
            Remote
          </button>
        </div>
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
