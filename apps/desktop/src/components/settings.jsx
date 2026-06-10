import React, { useEffect, useState } from 'react';
import { RefreshCw } from 'lucide-react';
import {
  groupUsageHooksByConfig,
  normalizeUsageHookStatuses,
  usageHookBadgeTone,
  usageHookStatusLabel
} from '../usageHooks.js';
import { userSyncLabel } from '../userSkillsGitSync.js';
import { Badge, PageHeader, PathList } from './common.jsx';

export function SettingsPage({
  paths,
  preferences,
  status,
  usageHooks,
  userSkillsGit,
  onInstallUsageHook,
  onOpenUsageHookConfig,
  onRefreshUsageHooks,
  onSaveRemoteUpdateTimeout,
  onSaveStatusRefreshInterval,
  onSaveUserSkillsRemote
}) {
  return (
    <>
      <PageHeader
        eyebrow="Settings"
        title="Settings"
        subtitle="Review managed storage roots and deployment defaults."
      />

      <section className="settingsGrid">
        <ManagedRootsPanel paths={paths} />
        <UserSkillsGitSettingsPanel
          status={status}
          userSkillsGit={userSkillsGit}
          onSave={onSaveUserSkillsRemote}
        />
        <StatusRefreshSettingsPanel
          preferences={preferences}
          status={status}
          onSaveRemoteUpdateTimeout={onSaveRemoteUpdateTimeout}
          onSave={onSaveStatusRefreshInterval}
        />
        <UsageHookSettingsPanel
          status={status}
          usageHooks={usageHooks}
          onInstall={onInstallUsageHook}
          onOpenConfig={onOpenUsageHookConfig}
          onRefresh={onRefreshUsageHooks}
        />
      </section>
    </>
  );
}

function UsageHookSettingsPanel({ status, usageHooks, onInstall, onOpenConfig, onRefresh }) {
  const hookGroups = groupUsageHooksByConfig(normalizeUsageHookStatuses(usageHooks));
  const isInstalling = status === 'installing_usage_hook';

  return (
    <aside className="panel compactPanel usageHookSettingsPanel">
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

function StatusRefreshSettingsPanel({ preferences, status, onSave, onSaveRemoteUpdateTimeout }) {
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
    <aside className="panel compactPanel">
      <div className="panelHeader compact">
        <div>
          <h2>Status refresh</h2>
          <p>Dashboard status checks run automatically.</p>
        </div>
      </div>
      <form className="settingsForm" onSubmit={submit}>
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
    </aside>
  );
}

function UserSkillsGitSettingsPanel({ status, userSkillsGit, onSave }) {
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
    <aside className="panel compactPanel">
      <div className="panelHeader compact">
        <div>
          <h2>User skills Git</h2>
          <p>Shared repository used by every local user skill.</p>
        </div>
      </div>
      <form className="settingsForm" onSubmit={submit}>
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
    </aside>
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
