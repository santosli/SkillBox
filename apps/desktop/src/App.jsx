import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import desktopPackage from '../package.json';
import codexAppIcon from './assets/codex-app-icon.png';
import codexCliIcon from './assets/codex-cli-icon.png';

const filters = [
  { id: 'all', label: 'All' },
  { id: 'user', label: 'User' },
  { id: 'remote', label: 'Remote' }
];

const previewPaths = {
  root: '~/SkillBox',
  userSkillsRoot: '~/SkillBox/user-skills',
  remoteSkillsRoot: '~/SkillBox/remote-skills',
  databasePath: '~/SkillBox/skillbox.sqlite'
};

const previewPreferenceStorageKey = 'skillbox.skipLocalImportConfirmation';

const previewImportCandidates = [
  {
    name: 'personal-wiki-updater',
    description: 'Incrementally refresh the personal wiki derived layer.',
    sourcePath: '~/.agents/skills/personal-wiki-updater',
    sourceRoot: '~/.agents/skills',
    contentHash: '87b21f5571a7d332',
    suggestedType: 'user',
    skillType: 'user',
    suggestionReason: 'inside ~/.agents/skills',
    importOrigin: 'local-scan',
    isSelected: true,
    conflict: null
  },
  {
    name: 'find-skills',
    description: 'Discover and install agent skills from local and remote sources.',
    sourcePath: '~/.codex/skills/find-skills',
    sourceRoot: '~/.codex/skills',
    contentHash: 'a9c42f1dd4822c80',
    suggestedType: 'remote',
    skillType: 'remote',
    suggestionReason: 'inside ~/.codex/skills',
    importOrigin: 'local-scan',
    isSelected: true,
    conflict: null
  },
  {
    name: 'imagegen',
    description: 'Generate and edit raster images for Codex workflows.',
    sourcePath: '~/.codex/skills/.system/imagegen',
    sourceRoot: '~/.codex/skills/.system',
    contentHash: 'c31de80b7ad93412',
    suggestedType: 'remote',
    skillType: 'remote',
    suggestionReason: 'inside ~/.codex/skills/.system',
    importOrigin: 'local-scan',
    isSelected: true,
    conflict: null
  }
];

export default function App() {
  const [skills, setSkills] = useState([]);
  const [paths, setPaths] = useState(null);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [selectedName, setSelectedName] = useState('');
  const [page, setPage] = useState('dashboard');
  const [status, setStatus] = useState('idle');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [isFirstUse, setIsFirstUse] = useState(false);
  const [importReview, setImportReview] = useState({
    open: false,
    candidates: [],
    errors: []
  });
  const [preferences, setPreferences] = useState({
    skipLocalImportConfirmation: false
  });
  const [localImportConfirmation, setLocalImportConfirmation] = useState({
    open: false,
    candidates: [],
    dontShowAgain: false
  });
  const [remoteImport, setRemoteImport] = useState({
    open: false,
    mode: 'url',
    value: '',
    error: ''
  });
  const contentRef = useRef(null);

  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    if (contentRef.current) {
      contentRef.current.scrollTop = 0;
      contentRef.current.scrollLeft = 0;
    }
  }, [page, selectedName, filter]);

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();

    return skills.filter((skill) => {
      const matchesFilter = filter === 'all' || skill.type === filter;
      const matchesQuery =
        !normalized ||
        [skill.name, skill.description, skill.sourceRoot, skill.status, skill.type]
          .filter(Boolean)
          .some((value) => String(value).toLowerCase().includes(normalized));

      return matchesFilter && matchesQuery;
    });
  }, [skills, query, filter]);

  const selected = skills.find((skill) => skill.name === selectedName) || filtered[0] || skills[0];

  const counts = useMemo(
    () => ({
      total: skills.length,
      user: skills.filter((skill) => skill.type === 'user').length,
      remote: skills.filter((skill) => skill.type === 'remote').length,
      updates: skills.filter(hasAvailableUpdate).length
    }),
    [skills]
  );

  async function refresh() {
    setStatus('loading');
    setError('');

    try {
      if (!window.__TAURI_INTERNALS__) {
        throw new Error('Browser preview is mocking an empty managed store. Run inside Tauri to use the local skill bridge.');
      }

      const [state, storedPreferences] = await Promise.all([
        invoke('managed_state'),
        invoke('managed_preferences').catch(() => null)
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];

      setSkills(managedSkills);
      setPaths(normalizePaths(state.paths));
      setPreferences(normalizePreferences(storedPreferences));
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName(managedSkills[0]?.name || '');
      setStatus('ready');
    } catch (scanError) {
      setSkills([]);
      setPaths(previewPaths);
      setPreferences(readPreviewPreferences());
      setIsFirstUse(true);
      setSelectedName('');
      setError('');
      setNotice(scanError.message || 'Browser preview is mocking an empty managed store.');
      setStatus('prototype');
    }
  }

  async function scanForImportCandidates() {
    setStatus('scanning');
    setError('');
    setNotice('');

    try {
      if (!window.__TAURI_INTERNALS__) {
        setImportReview({
          open: true,
          candidates: applyPreviewImportStatuses(
            previewImportCandidates.map(normalizeImportCandidate),
            skills
          ),
          errors: []
        });
        setNotice('Browser preview is using mock scan candidates.');
        setStatus('prototype');
        return;
      }

      const scan = await invoke('scan_import_candidates');
      const candidates = (scan.candidates || []).map(normalizeImportCandidate);

      setImportReview({
        open: candidates.length > 0,
        candidates,
        errors: scan.errors || []
      });
      setNotice(candidates.length === 0 ? 'No new local skills found.' : '');
      setStatus('ready');
    } catch (scanError) {
      setError(scanError.message || 'Unable to scan local skill folders.');
      setStatus('ready');
    }
  }

  function openRemoteImport() {
    setError('');
    setNotice('');
    setImportReview((current) => ({ ...current, open: false }));
    setRemoteImport({
      open: true,
      mode: 'url',
      value: '',
      error: ''
    });
  }

  function closeRemoteImport() {
    setRemoteImport((current) => ({ ...current, open: false, error: '' }));
  }

  function updateRemoteImport(patch) {
    setRemoteImport((current) => ({ ...current, ...patch, error: '' }));
  }

  async function submitRemoteImport(event) {
    event.preventDefault();

    const value = remoteImport.value.trim();
    if (!value) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a skill URL or Markdown file path.' }));
      return;
    }

    if (remoteImport.mode === 'url' && !isHttpUrl(value)) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a full http(s) skill URL.' }));
      return;
    }

    if (remoteImport.mode === 'markdown' && !value.toLowerCase().endsWith('.md')) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a local Markdown file path ending in .md.' }));
      return;
    }

    if (!window.__TAURI_INTERNALS__) {
      setImportReview({
        open: true,
        candidates: [remoteImportCandidate(remoteImport.mode, value)],
        errors: []
      });
      setRemoteImport((current) => ({ ...current, open: false, value: '', error: '' }));
      setNotice('Browser preview is using a provided remote source.');
      setStatus('prototype');
      return;
    }

    try {
      if (remoteImport.mode === 'url') {
        await invoke('parse_github_url', { url: value });
        setNotice('Remote URL was accepted. Remote download/import is not wired yet.');
      } else {
        setNotice('Markdown file import is not wired yet.');
      }
    } catch (submitError) {
      setRemoteImport((current) => ({
        ...current,
        error: submitError.message || String(submitError) || 'Unable to prepare this import.'
      }));
      return;
    }

    setRemoteImport((current) => ({ ...current, open: false, value: '', error: '' }));
    setStatus('ready');
  }

  function closeImportReview() {
    setImportReview((current) => ({ ...current, open: false }));
  }

  function updateImportCandidate(sourcePath, patch) {
    setImportReview((current) => ({
      ...current,
      candidates: current.candidates.map((candidate) =>
        candidate.sourcePath === sourcePath ? { ...candidate, ...patch } : candidate
      )
    }));
  }

  function toggleAllImportCandidates() {
    setImportReview((current) => ({
      ...current,
      candidates: toggleImportCandidateSelection(current.candidates)
    }));
  }

  async function importSelectedCandidates() {
    const selected = importReview.candidates.filter((candidate) => candidate.isSelected && isImportableCandidate(candidate));
    if (selected.length === 0) {
      setNotice('Select at least one candidate without conflicts to import.');
      return;
    }

    if (shouldConfirmLocalImport(selected, preferences)) {
      setLocalImportConfirmation({
        open: true,
        candidates: selected,
        dontShowAgain: false
      });
      return;
    }

    await runCandidateImport(selected);
  }

  async function runCandidateImport(selected, noticePrefix = '') {
    setStatus('importing');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const importedSkills = selected.map(candidateToPreviewSkill);

      setSkills((current) => mergeSkills(current, importedSkills));
      setSelectedName(importedSkills[0]?.name || '');
      setIsFirstUse(false);
      setImportReview({ open: false, candidates: [], errors: [] });
      setStatus('prototype');
      setNotice(importNotice(noticePrefix, `Mock imported ${importedSkills.length} skills.`));
      return;
    }

    try {
      const result = await invoke('import_candidates', {
        items: selected.map((candidate) => ({
          source_path: candidate.sourcePath,
          skill_type: candidate.skillType,
          deploy_back_to_source: true
        }))
      });
      const importErrors = result.errors || [];

      setImportReview({ open: false, candidates: [], errors: [] });
      await refresh();
      setNotice(
        importNotice(
          noticePrefix,
          importErrors.length > 0
            ? `Imported ${result.imported?.length || 0} skills. ${importErrors.length} item failed.`
            : `Imported ${result.imported?.length || 0} skills.`
        )
      );
    } catch (importError) {
      setError(importError.message || 'Unable to import selected skills.');
      setStatus('ready');
    }
  }

  function closeLocalImportConfirmation() {
    if (status === 'importing') {
      return;
    }
    setLocalImportConfirmation({ open: false, candidates: [], dontShowAgain: false });
  }

  async function confirmLocalImport() {
    const selected = localImportConfirmation.candidates;
    let noticePrefix = '';

    if (localImportConfirmation.dontShowAgain) {
      try {
        await saveSkipLocalImportConfirmation(true);
      } catch (preferenceError) {
        noticePrefix = `Preference was not saved: ${preferenceError.message || String(preferenceError)}.`;
      }
    }

    setLocalImportConfirmation({ open: false, candidates: [], dontShowAgain: false });
    await runCandidateImport(selected, noticePrefix);
  }

  async function saveSkipLocalImportConfirmation(skip) {
    if (!window.__TAURI_INTERNALS__) {
      try {
        window.localStorage.setItem(previewPreferenceStorageKey, skip ? 'true' : 'false');
      } catch {
        // Browser preview can run without durable storage; keep the session preference in React state.
      }
      const nextPreferences = { skipLocalImportConfirmation: skip };
      setPreferences(nextPreferences);
      return nextPreferences;
    }

    const storedPreferences = await invoke('set_skip_local_import_confirmation', { skip });
    const nextPreferences = normalizePreferences(storedPreferences);
    setPreferences(nextPreferences);
    return nextPreferences;
  }

  function openDashboard(nextFilter = filter) {
    setFilter(nextFilter);
    setPage('dashboard');
  }

  function openSkill(skill) {
    setSelectedName(skill.name);
    setPage('detail');
  }

  return (
    <main className="appShell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brandMark">SB</div>
          <div>
            <strong>SkillBox</strong>
            <span>Local skill manager</span>
          </div>
        </div>

        <nav className="navGroup" aria-label="Primary">
          <NavButton active={(page === 'dashboard' && filter === 'all') || page === 'detail'} icon="dashboard" label="Dashboard" onClick={() => openDashboard('all')} />
          <NavButton active={page === 'dashboard' && filter === 'user'} icon="user-skills" label="User Skills" onClick={() => openDashboard('user')} />
          <NavButton active={page === 'dashboard' && filter === 'remote'} icon="remote-skills" label="Remote Skills" onClick={() => openDashboard('remote')} />
        </nav>

        <div className="sidebarFooter">
          <FooterButton active={page === 'settings'} icon="settings" label="Settings" onClick={() => setPage('settings')} />
          <FooterButton icon="help" label="Help" />
          <div className="sidebarVersion">
            <span>Version</span>
            <strong>v{desktopPackage.version}</strong>
          </div>
        </div>
      </aside>

      <section className="content" ref={contentRef}>
        {page === 'detail' && selected ? (
          <SkillDetail paths={paths} skill={selected} onBack={() => openDashboard('all')} onRefresh={refresh} />
        ) : page === 'settings' ? (
          <SettingsPage paths={paths} />
        ) : (
          <Dashboard
            counts={counts}
            error={error}
            filter={filter}
            filtered={filtered}
            isFirstUse={isFirstUse}
            notice={notice}
            query={query}
            status={status}
            onFilter={setFilter}
            onOpenSkill={openSkill}
            onQuery={setQuery}
            onInstall={openRemoteImport}
            onRefresh={scanForImportCandidates}
          />
        )}
      </section>

      {importReview.open ? (
        <ImportReview
          candidates={importReview.candidates}
          onClose={closeImportReview}
          onImport={importSelectedCandidates}
          onToggleAll={toggleAllImportCandidates}
          onToggleSelected={(candidate) =>
            isImportableCandidate(candidate)
              ? updateImportCandidate(candidate.sourcePath, { isSelected: !candidate.isSelected })
              : null
          }
          onTypeChange={(candidate, skillType) => updateImportCandidate(candidate.sourcePath, { skillType })}
          status={status}
        />
      ) : null}

      {remoteImport.open ? (
        <RemoteImportDialog
          error={remoteImport.error}
          mode={remoteImport.mode}
          status={status}
          value={remoteImport.value}
          onClose={closeRemoteImport}
          onModeChange={(mode) => updateRemoteImport({ mode, value: '' })}
          onSubmit={submitRemoteImport}
          onValueChange={(value) => updateRemoteImport({ value })}
        />
      ) : null}

      {localImportConfirmation.open ? (
        <LocalImportConfirmationDialog
          candidates={localImportConfirmation.candidates}
          dontShowAgain={localImportConfirmation.dontShowAgain}
          status={status}
          onClose={closeLocalImportConfirmation}
          onConfirm={confirmLocalImport}
          onDontShowAgainChange={(dontShowAgain) =>
            setLocalImportConfirmation((current) => ({ ...current, dontShowAgain }))
          }
        />
      ) : null}
    </main>
  );
}

function Dashboard({
  counts,
  error,
  filter,
  filtered,
  isFirstUse,
  notice,
  query,
  status,
  onFilter,
  onInstall,
  onOpenSkill,
  onQuery,
  onRefresh
}) {
  return (
    <>
      {!isFirstUse ? (
        <div className="dashboardActions" aria-label="Dashboard actions">
          <button className="button secondary" type="button" onClick={onRefresh}>
            Scan
          </button>
          <button className="button primary" type="button" onClick={onInstall}>
            Install skill
          </button>
        </div>
      ) : null}

      {error ? <div className="notice">{error}</div> : null}
      {notice ? <div className="notice success">{notice}</div> : null}

      {isFirstUse ? (
        <FirstUseDashboard status={status} onInstall={onInstall} onScan={onRefresh} />
      ) : (
        <>
      <section className="metrics" aria-label="Skill statistics">
        <Metric hint="Indexed locally" label="All skills" value={counts.total} tone="blue" />
        <Metric hint="Owned locally" label="User skills" value={counts.user} tone="green" />
        <Metric hint="GitHub-bound" label="Remote skills" value={counts.remote} tone="slate" />
        <Metric hint="New remote version" label="Available updates" value={counts.updates} tone="amber" />
      </section>

      <section className="dashboardGrid">
        <div className="panel allSkillsPanel">
          <div className="panelHeader">
            <div>
              <h2>All skills</h2>
              <p>{filtered.length} matching skills</p>
            </div>
            <span className={`runtime ${status}`}>{status}</span>
          </div>

          <div className="toolbar">
            <label className="searchField" aria-label="Search skills">
              <input
                value={query}
                onChange={(event) => onQuery(event.target.value)}
                placeholder="Search skills"
                type="search"
              />
            </label>

            <div className="segments" role="tablist" aria-label="Skill filter">
              {filters.map((item) => (
                <button
                  className={filter === item.id ? 'active' : ''}
                  key={item.id}
                  type="button"
                  onClick={() => onFilter(item.id)}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>

          <div className="skillsTable" role="table" aria-label="All skills">
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
                </span>
                <Badge tone={skill.type === 'user' ? 'green' : 'blue'}>{labelize(skill.type)}</Badge>
                <StatusBadge skill={skill} />
                <span className="checkedText">just now</span>
              </button>
            ))}

            {filtered.length === 0 ? (
              <div className="emptyState">
                <strong>No skills found</strong>
                <span>Try another filter or run a fresh scan.</span>
              </div>
            ) : null}
          </div>
        </div>

        <aside className="sideStack dashboardSideStack">
          <div className="panel compactPanel">
            <div className="panelHeader compact">
              <div>
                <h2>Recent activity</h2>
                <p>Local operations</p>
              </div>
            </div>
            <ActivityList />
          </div>
        </aside>
      </section>
        </>
      )}
    </>
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

function SettingsPage({ paths }) {
  return (
    <>
      <PageHeader
        eyebrow="Settings"
        title="Settings"
        subtitle="Review managed storage roots and deployment defaults."
      />

      <section className="settingsGrid">
        <ManagedRootsPanel paths={paths} />
      </section>
    </>
  );
}

function ManagedRootsPanel({ paths }) {
  return (
    <aside className="panel compactPanel">
      <div className="panelHeader compact">
        <div>
          <h2>Managed roots</h2>
          <p>Import will copy first, then replace runtime folders with symlinks.</p>
        </div>
      </div>
      <PathList
        items={[
          ['Managed root', paths?.root],
          ['User skills', paths?.userSkillsRoot],
          ['Remote skills', paths?.remoteSkillsRoot],
          ['Deploy mode', 'Copy, backup, symlink']
        ]}
      />
    </aside>
  );
}

function RemoteImportDialog({ error, mode, status, value, onClose, onModeChange, onSubmit, onValueChange }) {
  const isMarkdown = mode === 'markdown';

  return (
    <div className="modalBackdrop" role="presentation">
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

function LocalImportConfirmationDialog({
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
    <div className="modalBackdrop" role="presentation">
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

function ImportReview({ candidates, onClose, onImport, onToggleAll, onToggleSelected, onTypeChange, status }) {
  const selectableCount = candidates.filter(isImportableCandidate).length;
  const selectedCount = candidates.filter((candidate) => candidate.isSelected && isImportableCandidate(candidate)).length;
  const isAllSelected = selectableCount > 0 && selectedCount === selectableCount;

  return (
    <div className="modalBackdrop" role="presentation">
      <section className="importSheet" role="dialog" aria-modal="true" aria-labelledby="import-review-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="import-review-title">Import Review</h2>
            <p>Confirm each skill type before SkillBox copies it into the managed store.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close import review" onClick={onClose}>
            x
          </button>
        </div>

        <div className="candidateList">
          {candidates.map((candidate) => (
            <div className={candidateRowClass(candidate)} key={candidate.sourcePath}>
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
                  {candidate.importStatus === 'imported' ? <Badge tone="slate">Imported</Badge> : null}
                  {candidate.conflict ? <Badge tone="red">Conflict</Badge> : null}
                </div>
                <small>{candidate.description || 'No description in SKILL.md'}</small>
                <code>{compactPath(candidate.sourcePath)}</code>
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
          ))}
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

function SkillDetail({ paths, skill, onBack, onRefresh }) {
  const managedPath =
    skill.type === 'user'
      ? joinPath(paths?.userSkillsRoot, skill.name)
      : joinPath(paths?.remoteSkillsRoot, `${skill.name}/current`);
  const shortHash = (skill.contentHash || 'not-indexed').slice(0, 8);
  const operationStatus = skillStatus(skill);
  const statusFieldLabel = skill.type === 'remote' ? 'Update status' : 'Sync status';
  const statusPanelTitle = skill.type === 'remote' ? 'Update' : 'Sync';
  const statusPanelDetail = skill.type === 'remote' ? 'GitHub source check' : 'User-skills git status';

  return (
    <>
      <div className="detailHeader">
        <div>
          <button className="breadcrumb" type="button" onClick={onBack}>
            All skills / {skill.name}
          </button>
          <div className="titleLine">
            <h1>{skill.name}</h1>
            <Badge tone={skill.type === 'user' ? 'green' : 'blue'}>{labelize(skill.type)} skill</Badge>
          </div>
          <p>{skill.description || 'No description in SKILL.md frontmatter.'}</p>
        </div>
        <div className="headerActions">
          <button className="button secondary" type="button" onClick={onRefresh}>
            Check update
          </button>
          <button className="button secondary" type="button">
            Rollback
          </button>
          <button className="button primary" type="button">
            Deploy
          </button>
        </div>
      </div>

      <section className="detailGrid">
        <div className="panel detailMain">
          <div className="tabs" role="tablist" aria-label="Skill detail sections">
            <button className="active" type="button">
              Overview
            </button>
            <button type="button">Versions</button>
            <button type="button">Deployments</button>
            <button type="button">Logs</button>
          </div>

          <dl className="fieldGrid">
            <Field label="Managed path" value={managedPath} />
            <Field label="Source root" value={skill.sourceRoot} />
            <Field label="Current SHA" value={shortHash} />
            <Field label="Latest SHA" value={hasAvailableUpdate(skill) ? 'available' : 'not checked'} />
            <Field label="Skill file" value={skill.skillMdPath || joinPath(skill.path, 'SKILL.md')} />
            <Field label={statusFieldLabel} value={operationStatus.label} />
          </dl>

          <section className="subsection">
            <div className="subsectionHeader">
              <h2>Version history</h2>
              <span>{shortHash === 'not-inde' ? 'No indexed version' : '1 indexed version'}</span>
            </div>
            <div className="versionRows">
              <div className="versionRow">
                <span>
                  <strong>{shortHash}</strong>
                  <small>Current version</small>
                </span>
                <Badge tone="green">Active</Badge>
                <button type="button" disabled>
                  Current
                </button>
              </div>
              <div className="versionRow muted">
                <span>
                  <strong>Previous versions</strong>
                  <small>Rollback points will appear after updates are installed.</small>
                </span>
                <Badge tone="slate">Empty</Badge>
                <button type="button" disabled>
                  Rollback
                </button>
              </div>
            </div>
          </section>

          <section className="subsection">
            <div className="subsectionHeader">
              <h2>Recent operations</h2>
              <span>Local only</span>
            </div>
            <ActivityList compact />
          </section>
        </div>

        <aside className="sideStack">
          <StatusPanel title="Deployment" tone={isDeployed(skill) ? 'green' : 'amber'} value={isDeployed(skill) ? 'Healthy' : 'Not deployed'} detail="~/.codex/skills" />
          <StatusPanel title={statusPanelTitle} tone={operationStatus.tone} value={operationStatus.label} detail={statusPanelDetail} />
          <StatusPanel title="Symlink" tone={skill.isSymlink ? 'green' : 'slate'} value={skill.isSymlink ? 'Healthy' : 'Not linked'} detail={targetLabel(skill)} />
          <StatusPanel title="Source trust" tone="blue" value={labelize(skill.type)} detail={compactPath(skill.sourceRoot)} />
        </aside>
      </section>
    </>
  );
}

function PageHeader({ actions, eyebrow, subtitle, title }) {
  return (
    <header className="pageHeader">
      <div>
        <p className="eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
      {actions ? <div className="headerActions">{actions}</div> : null}
    </header>
  );
}

function NavButton({ active, icon, label, onClick }) {
  return (
    <button className={active ? 'navButton active' : 'navButton'} type="button" onClick={onClick}>
      <span className="navIcon">
        <Icon name={icon} />
      </span>
      {label}
    </button>
  );
}

function FooterButton({ active = false, icon, label, onClick }) {
  return (
    <button className={active ? 'active' : ''} type="button" onClick={onClick}>
      <Icon name={icon} />
      {label}
    </button>
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

function Icon({ name }) {
  if (name === 'setup') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M5 5h14v14H5z" />
        <path d="m9 12 2 2 4-5" />
      </svg>
    );
  }

  if (name === 'dashboard') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M4 5h7v7H4z" />
        <path d="M13 5h7v4h-7z" />
        <path d="M13 11h7v8h-7z" />
        <path d="M4 14h7v5H4z" />
      </svg>
    );
  }

  if (name === 'user-skills') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M16 19a4 4 0 0 0-8 0" />
        <circle cx="12" cy="8" r="3" />
        <path d="M4 5h2" />
        <path d="M18 5h2" />
        <path d="M4 12h2" />
        <path d="M18 12h2" />
      </svg>
    );
  }

  if (name === 'remote-skills') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="8" />
        <path d="M4 12h16" />
        <path d="M12 4a12 12 0 0 1 0 16" />
        <path d="M12 4a12 12 0 0 0 0 16" />
      </svg>
    );
  }

  if (name === 'settings') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 0 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 0 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 0 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3h.1a1.7 1.7 0 0 0 1-1.6V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1 1.6h.1a1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 0 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9v.1a1.7 1.7 0 0 0 1.6 1H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5.8Z" />
      </svg>
    );
  }

  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="9" />
      <path d="M9.8 9a2.4 2.4 0 0 1 4.5 1.2c0 1.7-2.1 2-2.1 3.5" />
      <path d="M12 17h.01" />
    </svg>
  );
}

function Metric({ hint, label, tone, value }) {
  return (
    <div className={`metric ${tone}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{hint}</small>
    </div>
  );
}

function Badge({ children, tone = 'slate' }) {
  return <span className={`badge ${tone}`}>{children}</span>;
}

function StatusBadge({ skill }) {
  const status = skillStatus(skill);
  return <Badge tone={status.tone}>{status.label}</Badge>;
}

function Field({ label, value }) {
  return (
    <div className="field">
      <dt>{label}</dt>
      <dd>{value || 'Not available'}</dd>
    </div>
  );
}

function PathList({ items }) {
  return (
    <dl className="pathList">
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value || 'Not available'}</dd>
        </div>
      ))}
    </dl>
  );
}

function ActivityList({ compact = false }) {
  const items = [
    ['Scan completed', 'Runtime folders checked'],
    ['Managed layout ready', '~/SkillBox verified'],
    ['Symlink policy active', 'Deployments stay reversible']
  ];

  return (
    <ul className={compact ? 'activityList compact' : 'activityList'}>
      {items.map(([title, detail]) => (
        <li key={title}>
          <span />
          <div>
            <strong>{title}</strong>
            <small>{detail}</small>
          </div>
        </li>
      ))}
    </ul>
  );
}

function StatusPanel({ detail, title, tone, value }) {
  return (
    <div className="panel statusPanel">
      <span className={`statusLight ${tone}`} />
      <div>
        <p>{title}</p>
        <strong>{value}</strong>
        <small>{detail}</small>
      </div>
    </div>
  );
}

function normalizeSkill(skill) {
  const sourceRoot = skill.sourceRoot || skill.source_root;
  const isSymlink = skill.isSymlink || skill.is_symlink;
  const type = skill.type || inferType(sourceRoot);

  return {
    ...skill,
    sourceRoot,
    contentHash: skill.contentHash || skill.content_hash,
    skillMdPath: skill.skillMdPath || skill.skill_md_path,
    isSymlink,
    type,
    status: skill.status || defaultSkillStatus(type)
  };
}

function normalizePreferences(preferences) {
  return {
    skipLocalImportConfirmation: Boolean(
      preferences?.skipLocalImportConfirmation ?? preferences?.skip_local_import_confirmation
    )
  };
}

function readPreviewPreferences() {
  try {
    return {
      skipLocalImportConfirmation: window.localStorage.getItem(previewPreferenceStorageKey) === 'true'
    };
  } catch {
    return { skipLocalImportConfirmation: false };
  }
}

function normalizeImportCandidate(candidate) {
  const suggestedType = candidate.suggestedType || candidate.suggested_type || 'user';
  const sourcePath = candidate.sourcePath || candidate.source_path;
  const conflict = candidate.conflict || null;
  const importStatus = candidate.importStatus || candidate.import_status || 'importable';
  const isImportable = importStatus === 'importable' && !conflict;

  return {
    ...candidate,
    sourcePath,
    sourceRoot: candidate.sourceRoot || candidate.source_root,
    realPath: candidate.realPath || candidate.real_path,
    contentHash: candidate.contentHash || candidate.content_hash,
    suggestedType,
    skillType: candidate.skillType || candidate.skill_type || suggestedType,
    suggestionReason: candidate.suggestionReason || candidate.suggestion_reason || 'Needs confirm',
    importOrigin: candidate.importOrigin || candidate.import_origin || 'local-scan',
    importStatus,
    conflict,
    isSelected: isImportable
  };
}

function remoteImportCandidate(mode, value) {
  const name = inferSkillNameFromImportValue(value);
  const isMarkdown = mode === 'markdown';

  return {
    name,
    description: isMarkdown ? 'Remote skill created from a Markdown file.' : 'Remote skill source provided by URL.',
    sourcePath: value,
    sourceRoot: inferImportSourceRoot(value),
    contentHash: previewContentHash(value),
    suggestedType: 'remote',
    skillType: 'remote',
    suggestionReason: isMarkdown ? 'User provided Markdown file' : 'User provided skill URL',
    importOrigin: 'remote-input',
    importStatus: 'importable',
    isSelected: true,
    conflict: null
  };
}

function applyPreviewImportStatuses(candidates, importedSkills) {
  const importedHashes = new Set(importedSkills.map((skill) => skill.contentHash).filter(Boolean));
  const importedNames = new Set(importedSkills.map((skill) => skill.name).filter(Boolean));

  return candidates.map((candidate) => {
    if (!importedHashes.has(candidate.contentHash) && !importedNames.has(candidate.name)) {
      return candidate;
    }

    return {
      ...candidate,
      importStatus: 'imported',
      isSelected: false,
      suggestionReason: 'Imported; source links to SkillBox'
    };
  });
}

function shouldConfirmLocalImport(candidates, preferences) {
  if (preferences.skipLocalImportConfirmation) {
    return false;
  }

  return candidates.some((candidate) => isImportableCandidate(candidate) && requiresLocalImportConfirmation(candidate));
}

function requiresLocalImportConfirmation(candidate) {
  const sourcePath = String(candidate.sourcePath || '');

  if (candidate.importOrigin === 'remote-input') {
    return false;
  }

  if (isHttpUrl(sourcePath) || sourcePath.toLowerCase().endsWith('.md')) {
    return false;
  }

  return true;
}

function importNotice(prefix, message) {
  return [prefix, message].filter(Boolean).join(' ');
}

function isImportableCandidate(candidate) {
  return candidate.importStatus !== 'imported' && !candidate.conflict;
}

function candidateRowClass(candidate) {
  return [
    'candidateRow',
    candidate.conflict ? 'conflict' : '',
    candidate.importStatus === 'imported' ? 'imported' : ''
  ]
    .filter(Boolean)
    .join(' ');
}

function candidateStatusNote(candidate) {
  if (candidate.conflict) {
    return candidate.conflict;
  }
  if (candidate.importStatus === 'imported') {
    return '';
  }
  if (candidateSource(candidate)) {
    return '';
  }
  return candidate.suggestionReason;
}

function candidateSource(candidate) {
  const values = [
    candidate.sourceRoot,
    candidate.sourcePath,
    candidate.realPath,
    candidate.suggestionReason
  ]
    .filter(Boolean)
    .map((value) => String(value));
  const combined = values.join(' ');

  if (combined.includes('/.agents/skills') || combined.includes('~/.agents/skills')) {
    return { kind: 'agent', label: 'From ~/.agents/skills' };
  }

  if (combined.includes('/.codex/skills') || combined.includes('~/.codex/skills')) {
    return { kind: 'codex', label: 'From ~/.codex/skills' };
  }

  return null;
}

function toggleImportCandidateSelection(candidates) {
  const selectable = candidates.filter(isImportableCandidate);
  const shouldSelectAll = selectable.some((candidate) => !candidate.isSelected);

  return candidates.map((candidate) =>
    isImportableCandidate(candidate) ? { ...candidate, isSelected: shouldSelectAll } : candidate
  );
}

function isHttpUrl(value) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
}

function inferSkillNameFromImportValue(value) {
  const clean = value.split(/[?#]/)[0].replace(/\/+$/, '');
  const parts = clean.split(/[\\/]/).filter(Boolean);
  let name = parts[parts.length - 1] || 'remote-skill';

  if (name.toLowerCase() === 'skill.md' && parts.length > 1) {
    name = parts[parts.length - 2];
  } else if (name.toLowerCase().endsWith('.md')) {
    name = name.slice(0, -3);
  }

  return name || 'remote-skill';
}

function inferImportSourceRoot(value) {
  try {
    const parsed = new URL(value);
    const pathParts = parsed.pathname.split('/').filter(Boolean).slice(0, 2);
    return [parsed.hostname, ...pathParts].join('/');
  } catch {
    const clean = value.split(/[?#]/)[0].replace(/\/+$/, '');
    const parts = clean.split(/[\\/]/).filter(Boolean);
    return parts.slice(0, -1).join('/') || clean;
  }
}

function previewContentHash(value) {
  let hash = 0;
  for (const char of value) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return `preview-${hash.toString(16).padStart(8, '0')}`;
}

function candidateToPreviewSkill(candidate) {
  const type = candidate.skillType || candidate.suggestedType || 'user';
  const managedPath =
    type === 'user'
      ? joinPath(previewPaths.userSkillsRoot, candidate.name)
      : joinPath(previewPaths.remoteSkillsRoot, `${candidate.name}/current`);

  return {
    name: candidate.name,
    type,
    description: candidate.description,
    sourceRoot: candidate.sourceRoot,
    path: managedPath,
    skillMdPath: joinPath(managedPath, 'SKILL.md'),
    status: defaultSkillStatus(type),
    isSymlink: true,
    contentHash: candidate.contentHash
  };
}

function mergeSkills(current, imported) {
  const next = new Map(current.map((skill) => [skill.name, skill]));
  for (const skill of imported) {
    next.set(skill.name, skill);
  }
  return Array.from(next.values()).sort((left, right) => left.name.localeCompare(right.name));
}

function normalizePaths(paths) {
  if (!paths) return paths;

  return {
    ...paths,
    userSkillsRoot: paths.userSkillsRoot || paths.user_skills_root,
    remoteSkillsRoot: paths.remoteSkillsRoot || paths.remote_skills_root,
    databasePath: paths.databasePath || paths.database_path
  };
}

function inferType(sourceRoot = '') {
  if (String(sourceRoot).includes('.agents')) return 'user';
  return 'remote';
}

function isDeployed(skill) {
  return skill.isSymlink || ['deployed', 'ok', 'healthy'].includes(String(skill.status).toLowerCase());
}

function defaultSkillStatus(type) {
  return type === 'user' ? 'sync not checked' : 'update not checked';
}

function hasAvailableUpdate(skill) {
  const normalized = String(skill.status || '').toLowerCase();
  return skill.type === 'remote' && (normalized.includes('update available') || normalized.includes('new version'));
}

function skillStatus(skill) {
  const normalized = String(skill.status || '').toLowerCase();

  if (normalized.includes('error')) return { label: 'Error', tone: 'red' };
  if (normalized.includes('conflict')) return { label: 'Conflict', tone: 'red' };
  if (skill.type === 'remote') {
    if (hasAvailableUpdate(skill)) {
      return { label: 'Update available', tone: 'amber' };
    }
    if (normalized.includes('up to date') || normalized.includes('current') || normalized.includes('deployed') || normalized.includes('ok')) {
      return { label: 'Up to date', tone: 'green' };
    }
    return { label: 'Update not checked', tone: 'slate' };
  }

  if (normalized.includes('needs sync') || normalized.includes('dirty') || normalized.includes('sync needed')) {
    return { label: 'Needs sync', tone: 'amber' };
  }
  if (normalized.includes('synced') || normalized.includes('clean') || normalized.includes('up to date') || normalized.includes('ok')) {
    return { label: 'Synced', tone: 'green' };
  }
  return { label: 'Sync not checked', tone: 'slate' };
}

function labelize(value = '') {
  return String(value).replace(/[-_]/g, ' ');
}

function compactPath(value = '') {
  return String(value || 'Not available').replace('/Users/santos', '~');
}

function joinPath(root, child) {
  if (!root) return child;
  return `${String(root).replace(/\/$/, '')}/${child}`;
}

function targetLabel(skill) {
  if (skill.isSymlink || isDeployed(skill)) return '~/.codex/skills';
  return 'Not deployed';
}
