import React, { useEffect, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import {
  groupUsageHooksByConfig,
  normalizeUsageHookStatuses,
  usageHookBadgeTone,
  usageHookStatusLabel
} from '../usageHooks.js';
import { userSyncLabel } from '../userSkillsGitSync.js';
import { Badge, PageTitleRow, PathList } from './common.jsx';

const settingsSections = [
  { id: 'storage', label: 'Storage', href: '#settings-storage' },
  { id: 'sync', label: 'Sync', href: '#settings-sync' },
  { id: 'updates', label: 'Updates', href: '#settings-updates' },
  { id: 'hooks', label: 'Hooks', href: '#settings-hooks' }
];

export function SettingsPage({
  appUpdate,
  paths,
  preferences,
  status,
  usageHooks,
  userSkillsGit,
  onCheckAppUpdate,
  onInstallAppUpdate,
  onInstallUsageHook,
  onOpenUsageHookConfig,
  onRefreshUsageHooks,
  onSaveRemoteUpdateTimeout,
  onSaveStatusRefreshInterval,
  onSaveUserSkillsRemote
}) {
  const normalizedUsageHooks = normalizeUsageHookStatuses(usageHooks);
  const usageHookGroups = groupUsageHooksByConfig(normalizedUsageHooks);
  const [activeSettingsSection, setActiveSettingsSection] = useState('storage');

  return (
    <div className="settingsPage">
      <PageTitleRow title="Settings" />

      <section className="settingsWorkbench" aria-label="Settings workbench">
        <SettingsRail
          activeSettingsSection={activeSettingsSection}
          onSelectSection={setActiveSettingsSection}
        />
        <div className="settingsContent">
          <ManagedRootsPanel paths={paths} />
          <SyncRefreshSettingsPanel
            preferences={preferences}
            status={status}
            userSkillsGit={userSkillsGit}
            onSaveRemoteUpdateTimeout={onSaveRemoteUpdateTimeout}
            onSaveStatusRefreshInterval={onSaveStatusRefreshInterval}
            onSaveUserSkillsRemote={onSaveUserSkillsRemote}
          />
          <AppUpdateSettingsPanel
            appUpdate={appUpdate}
            onCheck={onCheckAppUpdate}
            onInstall={onInstallAppUpdate}
          />
          <UsageHookSettingsPanel
            hookGroups={usageHookGroups}
            status={status}
            onInstall={onInstallUsageHook}
            onOpenConfig={onOpenUsageHookConfig}
            onRefresh={onRefreshUsageHooks}
          />
        </div>
      </section>
    </div>
  );
}

function SettingsRail({ activeSettingsSection, onSelectSection }) {
  return (
    <aside className="settingsRail" aria-label="Settings navigation">
      <nav className="settingsRailNav" aria-label="Settings sections">
        {settingsSections.map((item) => (
          <a
            aria-current={activeSettingsSection === item.id ? 'true' : undefined}
            className={activeSettingsSection === item.id ? 'active' : ''}
            href={item.href}
            key={item.id}
            onClick={() => onSelectSection(item.id)}
          >
            {item.label}
          </a>
        ))}
      </nav>
    </aside>
  );
}

function AppUpdateSettingsPanel({ appUpdate, onCheck, onInstall }) {
  const isChecking = appUpdate?.state === 'checking';
  const isInstalling = appUpdate?.state === 'installing';
  const isDisabled = appUpdate?.state === 'disabled';
  const hasUpdate = Boolean(appUpdate?.available && appUpdate?.version);
  const message = appUpdate?.message || (hasUpdate ? `Version ${appUpdate.version} is ready.` : '');

  return (
    <aside className="panel compactPanel appUpdateSettingsPanel settingsPanel" id="settings-updates">
      <div className="panelHeader compact">
        <div>
          <h2>App updates</h2>
          <p>Check signed GitHub Releases before installing.</p>
        </div>
      </div>
      <div className="settingsForm">
        <PathList
          items={[
            ['Current version', appUpdate?.currentVersion ? `v${appUpdate.currentVersion}` : 'Unknown'],
            ['Available version', hasUpdate ? `v${appUpdate.version}` : 'None'],
            ['Last checked', appUpdate?.checkedAt || 'Not checked']
          ]}
        />
        {appUpdate?.body ? <pre className="appUpdateNotes">{appUpdate.body}</pre> : null}
        <div className="settingsActions">
          {message ? (
            <span className={appUpdate?.state === 'error' ? 'settingsError' : 'settingsSaved'}>
              {message}
            </span>
          ) : (
            <span />
          )}
          <div className="appUpdateActions">
            <button
              className="button secondary"
              disabled={isChecking || isInstalling || isDisabled}
              type="button"
              onClick={onCheck}
            >
              {isChecking ? 'Checking...' : 'Check for updates'}
            </button>
            <button
              className="button primary"
              disabled={!hasUpdate || isChecking || isInstalling || isDisabled}
              type="button"
              onClick={onInstall}
            >
              {isInstalling ? 'Installing...' : 'Install and restart'}
            </button>
          </div>
        </div>
      </div>
    </aside>
  );
}

function UsageHookSettingsPanel({ hookGroups, status, onInstall, onOpenConfig, onRefresh }) {
  const isInstalling = status === 'installing_usage_hook';

  return (
    <aside className="panel compactPanel usageHookSettingsPanel settingsPanel" id="settings-hooks">
      <div className="panelHeader compact">
        <div>
          <h2>Usage hook injection</h2>
          <p>Record real agent skill calls from runtime hooks.</p>
        </div>
        <div className="panelActions">
          <button
            aria-label="Refresh usage hook status"
            className="iconButton"
            disabled={isInstalling}
            title="Refresh usage hook status"
            type="button"
            onClick={onRefresh}
          >
            <RefreshCw aria-hidden="true" />
          </button>
        </div>
      </div>
      <div className="usageHookList">
        {hookGroups.map((group) => (
          <div className="usageHookRow" key={group.key}>
            <div className="usageHookMain">
              <strong>{group.label}</strong>
              <small>{group.configPath || 'Config path unavailable'}</small>
              <code>{group.command || '~/.skillbox/bin/skillbox-usage-hook'}</code>
            </div>
            <div className="usageHookActions">
              <Badge tone={usageHookBadgeTone(group)}>
                {usageHookStatusLabel(group)}
              </Badge>
              <button
                className="button secondary"
                disabled={isInstalling || (group.installed && !group.configPath)}
                type="button"
                onClick={() =>
                  group.installed ? onOpenConfig(group.configPath) : onInstall(group.target)
                }
              >
                {isInstalling ? 'Injecting...' : group.installed ? 'Open' : 'Inject'}
              </button>
            </div>
            {group.activationNote ? (
              <small className="usageHookTrustNote">{group.activationNote}</small>
            ) : null}
          </div>
        ))}
      </div>
    </aside>
  );
}

function SyncRefreshSettingsPanel({
  preferences,
  status,
  userSkillsGit,
  onSaveRemoteUpdateTimeout,
  onSaveStatusRefreshInterval,
  onSaveUserSkillsRemote
}) {
  return (
    <aside className="panel compactPanel settingsPanel syncRefreshSettingsPanel" id="settings-sync">
      <div className="panelHeader compact">
        <div>
          <h2>Sync & refresh</h2>
          <p>Keep user skills backed by Git and status checks current.</p>
        </div>
      </div>
      <div className="syncRefreshGrid">
        <UserSkillsGitSettingsForm
          status={status}
          userSkillsGit={userSkillsGit}
          onSave={onSaveUserSkillsRemote}
        />
        <StatusRefreshSettingsForm
          preferences={preferences}
          status={status}
          onSaveRemoteUpdateTimeout={onSaveRemoteUpdateTimeout}
          onSave={onSaveStatusRefreshInterval}
        />
      </div>
    </aside>
  );
}

function StatusRefreshSettingsForm({ preferences, status, onSave, onSaveRemoteUpdateTimeout }) {
  const [intervalMinutes, setIntervalMinutes] = useState(
    String(preferences.statusRefreshIntervalMinutes || 5)
  );
  const [timeoutSeconds, setTimeoutSeconds] = useState(
    String(preferences.remoteUpdateTimeoutSeconds || 30)
  );
  const [saveStatus, setSaveStatus] = useState('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    setIntervalMinutes(String(preferences.statusRefreshIntervalMinutes || 5));
    setTimeoutSeconds(String(preferences.remoteUpdateTimeoutSeconds || 30));
  }, [preferences.statusRefreshIntervalMinutes, preferences.remoteUpdateTimeoutSeconds]);

  async function submit(event) {
    event.preventDefault();
    setSaveStatus('saving');
    setMessage('');

    try {
      await onSave(Number(intervalMinutes));
      await onSaveRemoteUpdateTimeout(Number(timeoutSeconds));
      setSaveStatus('saved');
      setMessage('Saved.');
    } catch (error) {
      setSaveStatus('error');
      setMessage(error.message || String(error) || 'Unable to save refresh interval.');
    }
  }

  return (
    <form className="settingsForm settingsSubform" onSubmit={submit}>
      <div className="settingsSubformHeader">
        <h3>Status refresh</h3>
        <p>Dashboard checks run automatically.</p>
      </div>
      <label className="remoteImportField">
        <span>Auto refresh interval</span>
        <div className="numberFieldRow">
          <input
            min="1"
            max="1440"
            step="1"
            type="number"
            value={intervalMinutes}
            onChange={(event) => {
              setIntervalMinutes(event.target.value);
              setMessage('');
            }}
          />
          <span>minutes</span>
        </div>
      </label>
      <label className="remoteImportField">
        <span>Git check timeout</span>
        <div className="numberFieldRow">
          <input
            min="5"
            max="300"
            step="1"
            type="number"
            value={timeoutSeconds}
            onChange={(event) => {
              setTimeoutSeconds(event.target.value);
              setMessage('');
            }}
          />
          <span>seconds</span>
        </div>
      </label>
      <div className="settingsActions">
        {message ? <span className={saveStatus === 'error' ? 'settingsError' : 'settingsSaved'}>{message}</span> : <span />}
        <button className="button primary" disabled={status === 'checking' || saveStatus === 'saving'} type="submit">
          {saveStatus === 'saving' ? 'Saving...' : 'Save status settings'}
        </button>
      </div>
    </form>
  );
}

function UserSkillsGitSettingsForm({ status, userSkillsGit, onSave }) {
  const [remoteUrl, setRemoteUrl] = useState(userSkillsGit.remoteUrl || '');
  const [saveStatus, setSaveStatus] = useState('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    setRemoteUrl(userSkillsGit.remoteUrl || '');
  }, [userSkillsGit.remoteUrl]);

  async function submit(event) {
    event.preventDefault();
    setSaveStatus('saving');
    setMessage('');

    try {
      await onSave(remoteUrl);
      setSaveStatus('saved');
      setMessage('Saved.');
    } catch (error) {
      setSaveStatus('error');
      setMessage(error.message || String(error) || 'Unable to save remote URL.');
    }
  }

  return (
    <form className="settingsForm settingsSubform" onSubmit={submit}>
      <div className="settingsSubformHeader">
        <h3>User skills Git</h3>
        <p>Shared repository used by every local user skill.</p>
      </div>
      <label className="remoteImportField">
        <span>Remote URL</span>
        <input
          placeholder="git@github.com:santosli/user-skills.git"
          value={remoteUrl}
          onChange={(event) => setRemoteUrl(event.target.value)}
        />
      </label>
      <PathList
        items={[
          ['Repository', userSkillsGit.repoPath || '~/.skillbox/user-skills'],
          ['Branch', userSkillsGit.branch || 'main'],
          ['State', userSyncLabel(userSkillsGit)]
        ]}
      />
      <div className="settingsActions">
        {message ? <span className={saveStatus === 'error' ? 'settingsError' : 'settingsSaved'}>{message}</span> : <span />}
        <button className="button primary" disabled={status === 'syncing' || saveStatus === 'saving'} type="submit">
          {saveStatus === 'saving' ? 'Saving...' : 'Save remote'}
        </button>
      </div>
    </form>
  );
}

function ManagedRootsPanel({ paths }) {
  return (
    <aside className="panel compactPanel settingsPanel" id="settings-storage">
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
