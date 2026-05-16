import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import desktopPackage from '../package.json';

const fallbackSkills = [
  {
    name: 'find-skills',
    type: 'remote',
    description: 'Discover and install agent skills from local and remote sources.',
    sourceRoot: '~/.codex/skills',
    status: 'up to date',
    isSymlink: true,
    contentHash: 'a9c42f1dd4822c80'
  },
  {
    name: 'imagegen',
    type: 'remote',
    description: 'Generate and edit raster images for Codex workflows.',
    sourceRoot: '~/.codex/skills/.system',
    status: 'update available',
    contentHash: 'c31de80b7ad93412'
  },
  {
    name: 'lark-doc',
    type: 'user',
    description: 'Create, fetch, and update Lark documents through the local CLI.',
    sourceRoot: '~/.agents/skills',
    status: 'synced',
    contentHash: '18f4ed3e7280c219'
  },
  {
    name: 'personal-wiki-updater',
    type: 'user',
    description: 'Incrementally refresh the personal wiki derived layer.',
    sourceRoot: '~/.agents/skills',
    status: 'needs sync',
    contentHash: '87b21f5571a7d332'
  },
  {
    name: 'grill-me',
    type: 'remote',
    description: 'Stress-test plans and designs through structured questioning.',
    sourceRoot: '~/.codex/skills',
    status: 'up to date',
    isSymlink: true,
    contentHash: 'f39ad8c7ee410a60'
  }
];

const filters = [
  { id: 'all', label: 'All' },
  { id: 'user', label: 'User' },
  { id: 'remote', label: 'Remote' }
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
    setStatus('scanning');
    setError('');

    try {
      if (!window.__TAURI_INTERNALS__) {
        throw new Error('Browser preview is using sample data. Run inside Tauri to use the local skill bridge.');
      }

      const [scan, managedPaths] = await Promise.all([invoke('scan_skills'), invoke('managed_paths')]);
      const scannedSkills = scan.skills?.map(normalizeSkill) || [];

      setSkills(scannedSkills);
      setPaths(normalizePaths(managedPaths));
      setSelectedName(scannedSkills[0]?.name || '');
      setStatus('ready');
    } catch (scanError) {
      setSkills(fallbackSkills);
      setPaths({
        root: '~/SkillBox',
        userSkillsRoot: '~/SkillBox/user-skills',
        remoteSkillsRoot: '~/SkillBox/remote-skills',
        databasePath: '~/SkillBox/skillbox.sqlite'
      });
      setSelectedName(fallbackSkills[0].name);
      setError(scanError.message || 'Desktop bridge is not available yet.');
      setStatus('prototype');
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
          <NavButton active={page === 'setup'} icon="setup" label="Getting Started" onClick={() => setPage('setup')} />
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
        {page === 'setup' ? (
          <GettingStarted paths={paths} status={status} onStartScan={refresh} />
        ) : page === 'detail' && selected ? (
          <SkillDetail paths={paths} skill={selected} onBack={() => openDashboard('all')} onRefresh={refresh} />
        ) : (
          <Dashboard
            counts={counts}
            error={error}
            filter={filter}
            filtered={filtered}
            paths={paths}
            query={query}
            status={status}
            onFilter={setFilter}
            onOpenSkill={openSkill}
            onQuery={setQuery}
            onRefresh={refresh}
          />
        )}
      </section>
    </main>
  );
}

function Dashboard({ counts, error, filter, filtered, paths, query, status, onFilter, onOpenSkill, onQuery, onRefresh }) {
  return (
    <>
      <PageHeader
        eyebrow="Dashboard"
        title="All skills"
        subtitle="Manage local and remote skills from one managed library."
        actions={
          <>
            <button className="button secondary" type="button" onClick={onRefresh}>
              Scan
            </button>
            <button className="button primary" type="button">
              Install skill
            </button>
          </>
        }
      />

      <section className="metrics" aria-label="Skill statistics">
        <Metric hint="Indexed locally" label="All skills" value={counts.total} tone="blue" />
        <Metric hint="Owned locally" label="User skills" value={counts.user} tone="green" />
        <Metric hint="GitHub-bound" label="Remote skills" value={counts.remote} tone="slate" />
        <Metric hint="New remote version" label="Available updates" value={counts.updates} tone="amber" />
      </section>

      {error ? <div className="notice">{error}</div> : null}

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
  );
}

function GettingStarted({ paths, status, onStartScan }) {
  const steps = [
    ['Choose managed root', paths?.root || '~/SkillBox', 'Ready'],
    ['Scan existing skill folders', '~/.codex/skills and ~/.agents/skills', status === 'scanning' ? 'Scanning' : 'Ready'],
    ['Classify local skills', 'Review user and remote skills before migration', 'Review'],
    ['Deploy with symlinks', 'Runtime folders point back to SkillBox', 'Pending']
  ];

  return (
    <>
      <PageHeader
        eyebrow="First run"
        title="Getting Started"
        subtitle="Set up your local skill library before migration and deployment."
        actions={
          <>
            <button className="button secondary" type="button">
              Configure paths
            </button>
            <button className="button primary" type="button" onClick={onStartScan}>
              Start scan
            </button>
          </>
        }
      />

      <section className="setupGrid">
        <div className="panel setupPanel">
          <div className="panelHeader">
            <div>
              <h2>Prepare SkillBox</h2>
              <p>Four checks before the first import.</p>
            </div>
          </div>

          <div className="setupSteps">
            {steps.map(([title, description, stepStatus], index) => (
              <div className="setupStep" key={title}>
                <span className={index < 2 ? 'stepStatus done' : 'stepStatus'}>{String(index + 1).padStart(2, '0')}</span>
                <div>
                  <strong>{title}</strong>
                  <small>{description}</small>
                </div>
                <Badge tone={index < 2 ? 'green' : 'amber'}>{stepStatus}</Badge>
              </div>
            ))}
          </div>
        </div>

        <aside className="panel summaryPanel">
          <div className="panelHeader compact">
            <div>
              <h2>Setup summary</h2>
              <p>Default managed layout</p>
            </div>
          </div>
          <PathList
            items={[
              ['Managed root', paths?.root],
              ['User skills', paths?.userSkillsRoot],
              ['Remote skills', paths?.remoteSkillsRoot],
              ['Deploy mode', 'Symlink']
            ]}
          />
        </aside>
      </section>
    </>
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
      <div className="headerActions">{actions}</div>
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
