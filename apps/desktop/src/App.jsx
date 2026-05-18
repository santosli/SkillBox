import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import desktopPackage from '../package.json';

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
    isSelected: false,
    conflict: null
  }
];

const previewGithubCandidate = {
  name: 'github-skill-kit',
  description: 'Mock GitHub skill installed from a normalized owner/repo/path ref.',
  sourcePath: 'https://github.com/santosli/skills/tree/main/github-skill-kit',
  sourceRoot: 'github.com/santosli/skills',
  contentHash: '4f6a91c2d0ab4421',
  suggestedType: 'remote',
  skillType: 'remote',
  suggestionReason: 'GitHub source metadata found',
  isSelected: true,
  conflict: null
};

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

      const state = await invoke('managed_state');
      const managedSkills = state.skills?.map(normalizeSkill) || [];

      setSkills(managedSkills);
      setPaths(normalizePaths(state.paths));
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName(managedSkills[0]?.name || '');
      setStatus('ready');
    } catch (scanError) {
      setSkills([]);
      setPaths(previewPaths);
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
          candidates: previewImportCandidates,
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

  function installFromGitHub() {
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      setImportReview({
        open: true,
        candidates: [previewGithubCandidate],
        errors: []
      });
      setNotice('Browser preview is using a mock GitHub skill.');
      setStatus('prototype');
      return;
    }

    setNotice('GitHub install flow is not wired yet.');
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

  async function importSelectedCandidates() {
    const selected = importReview.candidates.filter((candidate) => candidate.isSelected && !candidate.conflict);
    if (selected.length === 0) {
      setNotice('Select at least one candidate without conflicts to import.');
      return;
    }

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
      setNotice(`Mock imported ${importedSkills.length} skills.`);
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
        importErrors.length > 0
          ? `Imported ${result.imported?.length || 0} skills. ${importErrors.length} item failed.`
          : `Imported ${result.imported?.length || 0} skills.`
      );
    } catch (importError) {
      setError(importError.message || 'Unable to import selected skills.');
      setStatus('ready');
    }
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
        <div className="windowControls" aria-hidden="true">
          <span className="close" />
          <span className="minimize" />
          <span className="zoom" />
        </div>

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
          <FooterButton icon="settings" label="Settings" />
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
        ) : (
          <Dashboard
            counts={counts}
            error={error}
            filter={filter}
            filtered={filtered}
            isFirstUse={isFirstUse}
            notice={notice}
            paths={paths}
            query={query}
            status={status}
            onFilter={setFilter}
            onOpenSkill={openSkill}
            onQuery={setQuery}
            onInstall={installFromGitHub}
            onRefresh={scanForImportCandidates}
          />
        )}
      </section>

      {importReview.open ? (
        <ImportReview
          candidates={importReview.candidates}
          onClose={closeImportReview}
          onImport={importSelectedCandidates}
          onToggleSelected={(candidate) =>
            updateImportCandidate(candidate.sourcePath, { isSelected: !candidate.isSelected })
          }
          onTypeChange={(candidate, skillType) => updateImportCandidate(candidate.sourcePath, { skillType })}
          status={status}
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
  paths,
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
      <PageHeader
        eyebrow="Dashboard"
        title="All skills"
        subtitle="Manage local and remote skills from one managed library."
        actions={
          isFirstUse ? null : (
          <>
            <button className="button secondary" type="button" onClick={onRefresh}>
              Scan
            </button>
            <button className="button primary" type="button" onClick={onInstall}>
              Install skill
            </button>
          </>
          )
        }
      />

      {error ? <div className="notice">{error}</div> : null}
      {notice ? <div className="notice success">{notice}</div> : null}

      {isFirstUse ? (
        <FirstUseDashboard paths={paths} status={status} onInstall={onInstall} onScan={onRefresh} />
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

        <aside className="sideStack">
          <div className="panel compactPanel">
            <div className="panelHeader compact">
              <div>
                <h2>Managed roots</h2>
                <p>Single source of truth</p>
              </div>
            </div>
            <PathList
              items={[
                ['Root', paths?.root],
                ['User', paths?.userSkillsRoot],
                ['Remote', paths?.remoteSkillsRoot],
                ['Database', paths?.databasePath]
              ]}
            />
          </div>

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

function FirstUseDashboard({ paths, status, onInstall, onScan }) {
  return (
    <section className="firstUseGrid">
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
            Install from GitHub
          </button>
        </div>
      </div>

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
    </section>
  );
}

function ImportReview({ candidates, onClose, onImport, onToggleSelected, onTypeChange, status }) {
  const selectedCount = candidates.filter((candidate) => candidate.isSelected && !candidate.conflict).length;

  return (
    <div className="modalBackdrop" role="presentation">
      <section className="importSheet" role="dialog" aria-modal="true" aria-labelledby="import-review-title">
        <div className="importSheetHeader">
          <div>
            <p className="eyebrow">Scan completed</p>
            <h2 id="import-review-title">Import Review</h2>
            <p>Confirm each skill type before SkillBox copies it into the managed store.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close import review" onClick={onClose}>
            x
          </button>
        </div>

        <div className="candidateList">
          {candidates.map((candidate) => (
            <div className={candidate.conflict ? 'candidateRow conflict' : 'candidateRow'} key={candidate.sourcePath}>
              <label className="candidateCheck">
                <input
                  checked={candidate.isSelected}
                  disabled={Boolean(candidate.conflict)}
                  type="checkbox"
                  onChange={() => onToggleSelected(candidate)}
                />
                <span />
              </label>

              <div className="candidateMain">
                <div className="candidateTitle">
                  <strong>{candidate.name}</strong>
                  <Badge tone={candidate.skillType === 'user' ? 'green' : 'blue'}>
                    {candidate.skillType === 'user' ? 'User skill' : 'Remote skill'}
                  </Badge>
                  {candidate.conflict ? <Badge tone="red">Conflict</Badge> : null}
                </div>
                <small>{candidate.description || 'No description in SKILL.md'}</small>
                <code>{compactPath(candidate.sourcePath)}</code>
                <p>{candidate.conflict || candidate.suggestionReason}</p>
              </div>

              <div className="candidateTypeSwitch" role="group" aria-label={`${candidate.name} type`}>
                <button
                  className={candidate.skillType === 'user' ? 'active' : ''}
                  disabled={Boolean(candidate.conflict)}
                  type="button"
                  onClick={() => onTypeChange(candidate, 'user')}
                >
                  User
                </button>
                <button
                  className={candidate.skillType === 'remote' ? 'active' : ''}
                  disabled={Boolean(candidate.conflict)}
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
          <span>{selectedCount} selected</span>
          <div className="headerActions">
            <button className="button secondary" type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={status === 'importing'} type="button" onClick={onImport}>
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

function FooterButton({ icon, label }) {
  return (
    <button type="button">
      <Icon name={icon} />
      {label}
    </button>
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

function normalizeImportCandidate(candidate) {
  const suggestedType = candidate.suggestedType || candidate.suggested_type || 'user';
  const sourcePath = candidate.sourcePath || candidate.source_path;

  return {
    ...candidate,
    sourcePath,
    sourceRoot: candidate.sourceRoot || candidate.source_root,
    contentHash: candidate.contentHash || candidate.content_hash,
    suggestedType,
    skillType: candidate.skillType || candidate.skill_type || suggestedType,
    suggestionReason: candidate.suggestionReason || candidate.suggestion_reason || 'Needs confirm',
    isSelected: candidate.isSelected ?? candidate.is_selected ?? true
  };
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
