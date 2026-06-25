import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const css = await readFile(new URL('./styles.css', import.meta.url), 'utf8');
const colorsCss = await readFile(new URL('./colors.css', import.meta.url), 'utf8');
const appSourcePaths = [
  './App.jsx',
  './components/dashboard.jsx',
  './components/common.jsx',
  './components/workspaces.jsx',
  './components/history.jsx',
  './components/settings.jsx',
  './components/importReview.jsx',
  './components/skillDetail.jsx',
  './components/remoteSkills.jsx',
  './components/userSkillsSync.jsx',
  './skills.js',
  './historyEntries.js',
  './usageHooks.js',
  './appUpdates.js',
  './preferences.js',
  './previewData.js',
  './importFlow.js'
];
const appSource = (
  await Promise.all(
    appSourcePaths.map((path) => readFile(new URL(path, import.meta.url), 'utf8'))
  )
).join('\n');
const mainSource = await readFile(new URL('./main.jsx', import.meta.url), 'utf8');
const tauriSource = await readFile(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const tauriMainSource = await readFile(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');

test('dashboard and workspace cards fill the available row width while auto-wrapping', () => {
  const sharedGridRule = css.match(/\.skillCardGrid,\s*\.workspaceCardGrid\s*\{(?<body>[^}]*)\}/s)
    ?.groups.body || '';

  assert.match(css, /--dashboard-card-width:\s*360px;/);
  assert.match(css, /--dashboard-card-track:\s*minmax\(min\(100%,\s*var\(--dashboard-card-width\)\),\s*1fr\);/);
  assert.match(
    sharedGridRule,
    /grid-template-columns:\s*repeat\(auto-fill,\s*var\(--dashboard-card-track\)\);/
  );
  assert.match(sharedGridRule, /justify-content:\s*stretch;/);
  assert.doesNotMatch(sharedGridRule, /justify-content:\s*start;/);
  assert.doesNotMatch(sharedGridRule, /repeat\([234],\s*minmax\(0,\s*1fr\)\)/);
  assert.doesNotMatch(css, /\.skillCardGrid,\s*\.workspaceCardGrid\s*\{[^}]*repeat\([234],\s*minmax\(0,\s*1fr\)\)/s);
});

test('sidebar footer icons use the same shell as primary nav icons', () => {
  const sharedIconRule = css.match(
    /\.navButton \.navIcon,\s*\.sidebarFooter button \.footerIcon\s*\{(?<body>[^}]*)\}/s
  )?.groups.body || '';
  const sharedSvgRule = css.match(
    /\.navButton \.navIcon svg,\s*\.sidebarFooter button \.footerIcon svg\s*\{(?<body>[^}]*)\}/s
  )?.groups.body || '';

  assert.match(sharedIconRule, /width:\s*22px;/);
  assert.match(sharedIconRule, /height:\s*22px;/);
  assert.match(sharedIconRule, /border:\s*1px solid var\(--skillbox-border\);/);
  assert.match(sharedSvgRule, /width:\s*15px;/);
  assert.match(sharedSvgRule, /height:\s*15px;/);
  assert.doesNotMatch(css, /\.sidebarFooter button svg\s*\{[^}]*width:\s*22px;/s);
});

test('sidebar brand does not render a subtitle', () => {
  const brandRule = css.match(/\.brand\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const brandTextRule = css.match(/\.brand > div\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const brandTitleRule = css.match(/\.brand strong\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(appSource, /<strong>SkillBox<\/strong>/);
  assert.doesNotMatch(appSource, /Local skill manager/);
  assert.doesNotMatch(css, /\.brand span/);
  assert.match(brandRule, /gap:\s*9px;/);
  assert.match(brandTextRule, /min-height:\s*36px;/);
  assert.match(brandTextRule, /align-items:\s*center;/);
  assert.match(brandTitleRule, /font-size:\s*21px;/);
  assert.match(brandTitleRule, /line-height:\s*36px;/);
});

test('settings exposes app update checks without downloading automatically', () => {
  assert.match(appSource, /function AppUpdateSettingsPanel/);
  assert.match(appSource, /Check for updates/);
  assert.match(appSource, /Install and restart/);
  assert.match(appSource, /invoke\('check_app_update'/);
  assert.match(appSource, /invoke\('install_app_update'/);
  assert.match(appSource, /shouldCheckAppUpdateOnStartup/);
  assert.doesNotMatch(appSource, /downloadAndInstall/);
});

test('settings page uses a workbench rail with status summary and section nav', () => {
  const railSource = appSource.match(/function SettingsRail[\s\S]*?function SettingsStatusRow/)?.[0] || '';

  assert.match(appSource, /className="settingsWorkbench"/);
  assert.match(appSource, /function SettingsRail/);
  assert.match(appSource, /className="settingsRailSummary"/);
  assert.match(appSource, /className="settingsRailNav"/);
  assert.ok(
    railSource.indexOf('className="settingsRailNav"') <
      railSource.indexOf('className="settingsRailSummary"')
  );
  assert.doesNotMatch(appSource, /System status/);
  assert.doesNotMatch(appSource, /<SettingsStatusRow label="Store"/);
  assert.match(appSource, /<SettingsStatusRow label="Git" value=\{userSyncLabel\(userSkillsGit\)\}/);
  assert.match(appSource, /<SettingsStatusRow label="Updates" value=\{appUpdateStatusLabel\(appUpdate\)\}/);
  assert.match(appSource, /<SettingsStatusRow label="Hooks" value=\{`\$\{injectedHookCount\}\/\$\{supportedHookCount\} injected`\}/);
  assert.match(appSource, /className="settingsStoreHint"/);
  assert.match(appSource, /href="#settings-storage"/);
  assert.match(appSource, /href="#settings-sync"/);
  assert.match(appSource, /href="#settings-updates"/);
  assert.match(appSource, /href="#settings-hooks"/);
});

test('settings sections are anchored and sync controls are grouped together', () => {
  assert.match(appSource, /id="settings-storage"/);
  assert.match(appSource, /id="settings-sync"/);
  assert.match(appSource, /id="settings-updates"/);
  assert.match(appSource, /id="settings-hooks"/);
  assert.match(appSource, /function SyncRefreshSettingsPanel/);
  assert.match(appSource, /<h2>Sync & refresh<\/h2>/);
  assert.match(appSource, /<UserSkillsGitSettingsForm/);
  assert.match(appSource, /<StatusRefreshSettingsForm/);
  assert.match(appSource, /onSaveUserSkillsRemote/);
  assert.match(appSource, /onSaveStatusRefreshInterval/);
  assert.match(appSource, /onSaveRemoteUpdateTimeout/);
});

test('settings workbench CSS defines a desktop rail and responsive fallback', () => {
  const workbenchRule = css.match(/\.settingsWorkbench\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const railRule = css.match(/\.settingsRail\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const railSummaryRule = css.match(/\.settingsRailSummary\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const syncRule = css.match(/\.syncRefreshGrid\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const responsiveRule = css.match(/@media \(max-width: 1180px\)\s*\{(?<body>[\s\S]*?)@media \(max-width: 1360px\)/)
    ?.groups.body || '';

  assert.match(workbenchRule, /grid-template-columns:\s*minmax\(220px,\s*240px\)\s+minmax\(0,\s*960px\);/);
  assert.match(workbenchRule, /max-width:\s*1220px;/);
  assert.match(railRule, /position:\s*sticky;/);
  assert.match(railRule, /top:\s*24px;/);
  assert.doesNotMatch(railSummaryRule, /border:/);
  assert.doesNotMatch(railSummaryRule, /box-shadow:/);
  assert.match(syncRule, /grid-template-columns:\s*minmax\(0,\s*1\.18fr\)\s+minmax\(260px,\s*0\.82fr\);/);
  assert.match(responsiveRule, /\.settingsWorkbench\s*\{[^}]*grid-template-columns:\s*1fr;/s);
  assert.match(responsiveRule, /\.settingsRail\s*\{[^}]*position:\s*static;/s);
  assert.match(responsiveRule, /\.settingsRailNav\s*\{[^}]*grid-template-columns:\s*repeat\(4,\s*minmax\(0,\s*1fr\)\);/s);
});

test('tauri desktop bridge registers app update commands and pending state', () => {
  assert.match(tauriSource, /struct PendingAppUpdate/);
  assert.match(tauriSource, /fn app_update_disabled_response/);
  assert.match(tauriSource, /async fn check_app_update/);
  assert.match(tauriSource, /async fn install_app_update/);
  assert.match(tauriSource, /tauri_plugin_updater::Builder::new\(\)\.build\(\)/);
  assert.match(tauriSource, /app\.manage\(PendingAppUpdate::default\(\)\)/);
});

test('dashboard actions stay in one equal segmented row', () => {
  const contentRule = css.match(/\.content\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const controlRowRule = css.match(/\.dashboardControlRow\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const typeTabsRule = css.match(/\.dashboardTypeTabs\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const activeTypeTabsRule = css.match(
    /\.dashboardTypeTabs button\.active,\s*\.viewSwitch button\.active\s*\{(?<body>[^}]*)\}/s
  )?.groups.body || '';
  const actionGroupRule = css.match(/\.dashboardActionGroup\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const indicatorRule = css.match(/\.dashboardActionIndicator\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(contentRule, /overflow-y:\s*auto;/);
  assert.match(contentRule, /scrollbar-gutter:\s*stable;/);
  assert.match(controlRowRule, /display:\s*grid;/);
  assert.match(
    controlRowRule,
    /grid-template-columns:\s*minmax\(260px,\s*1fr\)\s+max-content\s+max-content\s+max-content;/
  );
  assert.match(typeTabsRule, /width:\s*380px;/);
  assert.match(typeTabsRule, /grid-template-columns:\s*repeat\(4,\s*minmax\(0,\s*1fr\)\);/);
  assert.match(activeTypeTabsRule, /background:\s*var\(--skillbox-blue-bg\);/);
  assert.match(activeTypeTabsRule, /color:\s*var\(--skillbox-blue-text\);/);
  assert.match(actionGroupRule, /width:\s*330px;/);
  assert.match(actionGroupRule, /grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\);/);
  assert.match(css, /\.dashboardActionGroup\.previewing \.dashboardActionIndicator\s*\{[^}]*opacity:\s*1;/s);
  assert.match(indicatorRule, /opacity:\s*0;/);
  assert.match(indicatorRule, /transform:\s*translateX\(calc\(var\(--dashboard-action-index,\s*0\) \* 100%\)\);/);
  assert.match(indicatorRule, /transform 280ms cubic-bezier\(0\.2,\s*0\.8,\s*0\.2,\s*1\);/);
  assert.match(appSource, /label:\s*isChecking \? 'Refreshing' : 'Refresh'/);
  assert.match(appSource, /label:\s*'Import'/);
  assert.match(appSource, /label:\s*'Install'/);
  assert.match(appSource, /onMouseEnter=\{\(\) => setPreviewAction\(action\.id\)\}/);
  assert.match(appSource, /onBlur=\{\(event\) =>/);
  assert.match(appSource, /setPreviewAction\(null\);/);
});

test('dashboard content keeps a compact title offset from the window top', () => {
  const contentRule = css.match(/\.content\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(contentRule, /padding:\s*24px 48px 48px;/);
});

test('workspace type tabs use three columns without an empty slot', () => {
  const workspaceTypeTabsRule = css.match(/\.workspaceTypeTabs\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(workspaceTypeTabsRule, /width:\s*max-content;/);
  assert.match(workspaceTypeTabsRule, /grid-template-columns:\s*repeat\(3,\s*minmax\(112px,\s*max-content\)\);/);
  assert.doesNotMatch(workspaceTypeTabsRule, /repeat\(4,/);
});

test('remote source binding dialog keeps long candidate lists inside the viewport', () => {
  const dialogRule = css.match(/\.remoteImportDialog\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const formRule = css.match(/\.remoteImportForm\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const candidateListRule = css.match(/\.remoteSourceCandidateList\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(dialogRule, /max-height:\s*min\(760px,\s*calc\(100vh - 64px\)\);/);
  assert.match(dialogRule, /grid-template-rows:\s*auto minmax\(0,\s*1fr\);/);
  assert.match(formRule, /min-height:\s*0;/);
  assert.match(formRule, /overflow-y:\s*auto;/);
  assert.match(candidateListRule, /max-height:\s*min\(420px,\s*42vh\);/);
  assert.match(candidateListRule, /overflow-y:\s*auto;/);
});

test('import review shows imported candidates by default', () => {
  assert.match(
    appSource,
    /const \[isImportedExpanded,\s*setIsImportedExpanded\]\s*=\s*useState\(true\);/
  );
});

test('workspace cards show the shared workspace icon beside the workspace name', () => {
  const workspaceCard = appSource.match(
    /function WorkspaceCard\(\{ isBusy, workspace, onForget, onOpenSkills \}\)\s*\{(?<body>[\s\S]*?)\n\}/
  )?.groups.body || '';

  assert.match(workspaceCard, /<strong>\{workspace\.displayName\}<\/strong>/);
  assert.match(workspaceCard, /<AgentIconBadge agent=\{workspace\.agentIcon\}/);
  assert.match(css, /\.workspaceCardTitleRow > \.skillAgentIcon\s*\{[^}]*flex:\s*0 0 24px;/s);
});

test('workspace card icon tooltips can overflow card bounds', () => {
  const workspaceCardRule = css.match(/\.workspaceCard\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const workspaceHoverRule = css.match(
    /\.workspaceCard:hover,\s*\.workspaceCard:focus-within\s*\{(?<body>[^}]*)\}/s
  )?.groups.body || '';

  assert.match(workspaceCardRule, /overflow:\s*visible;/);
  assert.doesNotMatch(workspaceCardRule, /overflow:\s*hidden;/);
  assert.match(workspaceHoverRule, /z-index:\s*2;/);
});

test('remote source search starts after the binding dialog has painted', () => {
  const openSourceDialog = appSource.match(
    /async function openRemoteSourceDialog\(skill\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(openSourceDialog, /setRemoteSourceDialog\(/);
  assert.match(openSourceDialog, /await waitForNextPaint\(\);/);
  assert.match(openSourceDialog, /void searchRemoteSourceCandidates\(skill\.name\);/);
  assert.ok(
    openSourceDialog.indexOf('await waitForNextPaint();') <
      openSourceDialog.indexOf('void searchRemoteSourceCandidates(skill.name);')
  );
});

test('remote source search is presented as a non-blocking background task', () => {
  const openSourceDialog = appSource.match(
    /async function openRemoteSourceDialog\(skill\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(openSourceDialog, /searching:\s*true/);
  assert.match(appSource, /Searching Claude Marketplace in the background\./);
  assert.match(appSource, /You can paste a GitHub URL or close this dialog while\s+results load\./);
  assert.match(appSource, /className="iconButton" disabled=\{dialog\.loading\}/);
  assert.match(appSource, /disabled=\{dialog\.loading\}\s+placeholder=/);
  assert.doesNotMatch(appSource, /disabled=\{dialog\.loading \|\| dialog\.searching\}/);
});

test('remote source search command runs marketplace lookup off the command handler', () => {
  assert.match(tauriSource, /async fn find_remote_source_candidates/);
  assert.match(tauriSource, /tauri::async_runtime::spawn_blocking/);
});

test('remote update status command runs off the command handler', () => {
  const checkCommandStart = tauriSource.indexOf('async fn check_remote_skill_updates');
  const nextCommandStart = tauriSource.indexOf('#[tauri::command]', checkCommandStart + 1);
  const checkCommand = tauriSource.slice(checkCommandStart, nextCommandStart);

  assert.ok(checkCommandStart > 0);
  assert.match(checkCommand, /tauri::async_runtime::spawn_blocking/);
});

test('remote source bind validation commands run off the command handler', () => {
  for (const commandName of ['preview_remote_source_binding', 'bind_remote_source']) {
    const commandStart = tauriSource.indexOf(`async fn ${commandName}`);
    const nextCommandStart = tauriSource.indexOf('#[tauri::command]', commandStart + 1);
    const command = tauriSource.slice(commandStart, nextCommandStart);

    assert.ok(commandStart > 0, `${commandName} should be async`);
    assert.match(command, /tauri::async_runtime::spawn_blocking/, `${commandName} should spawn blocking work`);
  }
});

test('remote skill URL import installs GitHub skills through the desktop bridge', () => {
  const submitRemoteImport = appSource.match(
    /async function submitRemoteImport\(event\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(submitRemoteImport, /invoke\('install_github_remote_skill',\s*\{\s*request:\s*\{/);
  assert.match(submitRemoteImport, /source_url:\s*value/);
  assert.match(submitRemoteImport, /target_root:\s*null/);
  assert.match(submitRemoteImport, /actor:\s*'desktop'/);
  assert.match(submitRemoteImport, /await refresh\(\);/);
  assert.doesNotMatch(submitRemoteImport, /invoke\('parse_github_url'/);
  assert.doesNotMatch(appSource, /Remote download\/import is not wired yet\./);
});

test('remote skill URL import restores ready state when install fails', () => {
  const submitRemoteImport = appSource.match(
    /async function submitRemoteImport\(event\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';
  const catchBlock = submitRemoteImport.match(/catch \(submitError\) \{(?<body>[\s\S]*?)\n    \}/)
    ?.groups.body || '';

  assert.match(catchBlock, /setStatus\('ready'\);/);
});

test('remote GitHub install command runs off the command handler', () => {
  const commandStart = tauriSource.indexOf('async fn install_github_remote_skill');
  const nextCommandStart = tauriSource.indexOf('#[tauri::command]', commandStart + 1);
  const command = tauriSource.slice(commandStart, nextCommandStart);

  assert.ok(commandStart > 0, 'install_github_remote_skill should be registered as a command');
  assert.match(command, /tauri::async_runtime::spawn_blocking/);
  assert.match(tauriSource, /install_github_remote_skill,/);
});

test('dashboard startup loads cached remote update state without refreshing', () => {
  assert.match(appSource, /invoke\('cached_remote_skill_updates'\)/);
  assert.match(appSource, /setRemoteSkillUpdates\(cachedRemoteUpdates\)/);
  assert.match(appSource, /setLastStatusCheckedAt\(cachedRemoteUpdates\.checkedAt \|\| ''\)/);
});

test('dashboard status refresh paints loading state before checking remotes', () => {
  const refreshStatuses = appSource.match(
    /async function refreshSkillStatuses\(\{ automatic = false, skillName = '' \} = \{\}\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(refreshStatuses, /setStatus\('checking'\);/);
  assert.match(refreshStatuses, /await waitForNextPaint\(\);/);
  assert.match(refreshStatuses, /invoke\('check_remote_skill_updates'/);
  assert.ok(
    refreshStatuses.indexOf('await waitForNextPaint();') <
      refreshStatuses.indexOf("invoke('check_remote_skill_updates'")
  );
});

test('dashboard refresh checks all remote skills while detail check targets one skill', () => {
  const refreshStatuses = appSource.match(
    /async function refreshSkillStatuses\(\{ automatic = false, skillName = '' \} = \{\}\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(refreshStatuses, /skillName\s*\?\s*invoke\('check_remote_skill_update'/);
  assert.match(refreshStatuses, /:\s*invoke\('check_remote_skill_updates'/);
  assert.match(appSource, /onRefreshStatuses=\{refreshSkillStatuses\}/);
  assert.match(appSource, /onCheckUpdates=\{\(\) => refreshSkillStatuses\(\{ skillName: selectedSkill\.name \}\)\}/);
});

test('remote update review uses the checked latest sha as preview target', () => {
  assert.match(
    appSource,
    /const selectedRemoteUpdate = selectedSkill\s*\?\s*remoteSkillUpdates\.statuses\.find\(\(item\) => item\.skillName === selectedSkill\.name\)\s*:\s*null;/
  );
  assert.match(appSource, /remoteUpdate=\{selectedRemoteUpdate\}/);
  assert.match(
    appSource,
    /onReviewUpdate=\{\(\) => openRemoteVersionReview\(selectedSkill, 'update', selectedRemoteUpdate\?\.latestSha \|\| ''\)\}/
  );
});

test('remote update checks pass the configured git timeout', () => {
  assert.match(appSource, /remoteUpdateTimeoutSeconds:\s*30/);
  assert.match(appSource, /remoteUpdateTimeoutSeconds: normalizeRemoteUpdateTimeoutSeconds/);
  assert.match(appSource, /timeoutSeconds:\s*preferences\.remoteUpdateTimeoutSeconds/);
  assert.match(tauriSource, /fn set_remote_update_timeout_seconds\(seconds: u32\)/);
  assert.match(tauriSource, /async fn check_remote_skill_update/);
});

test('dashboard refresh action shows an explicit loading affordance', () => {
  assert.match(appSource, /label:\s*isChecking \? 'Refreshing' : 'Refresh'/);
  assert.match(appSource, /loading:\s*isChecking/);
  assert.match(appSource, /aria-busy=\{action\.loading \? 'true' : undefined\}/);
  assert.match(appSource, /dashboardActionButton loading/);
  assert.match(css, /\.dashboardActionButton\.loading svg\s*\{[^}]*animation:\s*syncSpin 760ms linear infinite;/s);
});

test('remote update review starts after the loading dialog has painted', () => {
  const reviewDialog = appSource.match(
    /async function openRemoteVersionReview\(skill, action, targetVersion = ''\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(reviewDialog, /setRemoteVersionDialog\(/);
  assert.match(reviewDialog, /await waitForNextPaint\(\);/);
  assert.match(reviewDialog, /invoke\('preview_remote_version_change'/);
  assert.ok(
    reviewDialog.indexOf('await waitForNextPaint();') <
      reviewDialog.indexOf("invoke('preview_remote_version_change'")
  );
});

test('diff review dialogs keep large diffs inside the modal viewport', () => {
  const dialogRule = css.match(/\.gitCommitDialog\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const dialogBodyRule = css.match(/\.gitCommitDialogBody\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const formRule = css.match(/\.gitCommitForm\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const reviewRule = css.match(/\.gitCommitReview\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const formReviewRule = css.match(/\.gitCommitForm \.gitCommitReview\s*\{(?<body>[^}]*)\}/s)
    ?.groups.body || '';
  const filePaneRule = css.match(/\.gitFilePane\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const fileListRule = css.match(/\.gitFileList\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const diffPaneRule = css.match(/\.gitDiffPane\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const diffScrollerRule = css.match(/\.githubDiffScroller\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(dialogRule, /max-height:\s*min\(760px,\s*calc\(100vh - 64px\)\);/);
  assert.match(dialogRule, /grid-template-rows:\s*auto minmax\(0,\s*1fr\) auto;/);
  assert.match(dialogBodyRule, /min-height:\s*0;/);
  assert.match(dialogBodyRule, /overflow:\s*hidden;/);
  assert.match(formRule, /min-height:\s*0;/);
  assert.match(formRule, /overflow-y:\s*auto;/);
  assert.match(reviewRule, /min-height:\s*0;/);
  assert.match(formReviewRule, /height:\s*clamp\(260px,\s*calc\(100vh - 300px\),\s*430px\);/);
  assert.match(filePaneRule, /min-height:\s*0;/);
  assert.match(fileListRule, /min-height:\s*0;/);
  assert.match(fileListRule, /overflow-y:\s*auto;/);
  assert.match(diffPaneRule, /overflow:\s*hidden;/);
  assert.match(diffScrollerRule, /min-height:\s*0;/);
  assert.match(diffScrollerRule, /max-width:\s*100%;/);
  assert.match(appSource, /<div className="gitCommitDialogBody">/);
});

test('remote diff review footer separates actions from the diff pane edge', () => {
  const footerRule = css.match(/\.remoteDialogFooter\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(footerRule, /padding:\s*18px 24px 20px;/);
  assert.match(footerRule, /border-top:\s*1px solid #e5edf6;/);
  assert.match(footerRule, /background:\s*#ffffff;/);
});

test('remote update preview command runs off the command handler', () => {
  const previewCommandStart = tauriSource.indexOf('async fn preview_remote_version_change');
  const nextCommandStart = tauriSource.indexOf('#[tauri::command]', previewCommandStart + 1);
  const previewCommand = tauriSource.slice(previewCommandStart, nextCommandStart);

  assert.ok(previewCommandStart > 0);
  assert.match(previewCommand, /tauri::async_runtime::spawn_blocking/);
});

test('blocking desktop commands run off the command handler', () => {
  for (const commandName of [
    'sync_user_skills_git',
    'import_candidates',
    'apply_remote_version_change',
    'scan_workspaces',
    'record_skill_usage',
    'install_usage_hook',
    'list_history'
  ]) {
    const commandStart = tauriSource.indexOf(`async fn ${commandName}`);
    const nextCommandStart = tauriSource.indexOf('#[tauri::command]', commandStart + 1);
    const command = tauriSource.slice(commandStart, nextCommandStart);

    assert.ok(commandStart > 0, `${commandName} should be async`);
    assert.match(command, /tauri::async_runtime::spawn_blocking/, `${commandName} should spawn blocking work`);
  }
});

test('settings exposes usage hook injection for supported agents', () => {
  assert.match(appSource, /invoke\('usage_hook_statuses'\)/);
  assert.match(appSource, /invoke\('install_usage_hook'/);
  assert.match(appSource, /async function refreshUsageHookStatuses/);
  assert.match(appSource, /async function openUsageHookConfig\(path\)/);
  assert.match(appSource, /invoke\('open_local_file',\s*\{ path: configPath \}\)/);
  assert.match(appSource, /onOpenUsageHookConfig=\{openUsageHookConfig\}/);
  assert.match(appSource, /onRefreshUsageHooks=\{refreshUsageHookStatuses\}/);
  assert.match(appSource, /function UsageHookSettingsPanel/);
  assert.match(appSource, /onRefresh=\{onRefreshUsageHooks\}/);
  assert.match(appSource, /aria-label="Refresh usage hook status"/);
  assert.match(appSource, /function groupUsageHooksByConfig/);
  assert.match(appSource, /const normalizedUsageHooks = normalizeUsageHookStatuses\(usageHooks\);/);
  assert.match(appSource, /const usageHookGroups = groupUsageHooksByConfig\(normalizedUsageHooks\);/);
  assert.match(appSource, /hookGroups\.map/);
  assert.match(appSource, /group\.labels\.join\(' \/ '\)/);
  assert.match(appSource, /group\.installed \? onOpenConfig\(group\.configPath\) : onInstall\(group\.target\)/);
  assert.match(appSource, /group\.installed \? 'Open' : 'Inject'/);
  assert.match(appSource, /function usageHookStatusLabel/);
  assert.match(appSource, /Needs trust/);
  assert.match(appSource, /usageHookTrustNote/);
  assert.match(appSource, /trustRequired:\s*Boolean\(row\.trustRequired \?\? row\.trust_required\)/);
  assert.match(appSource, /activationNote:\s*row\.activationNote \|\| row\.activation_note \|\| ''/);
  assert.match(appSource, /Codex App/);
  assert.match(appSource, /Codex CLI/);
  assert.match(appSource, /Claude Code CLI/);
  assert.match(appSource, /Usage hook injection/);
  assert.match(tauriSource, /fn usage_hook_statuses/);
  assert.match(tauriSource, /async fn install_usage_hook/);
  assert.match(tauriSource, /fn open_local_file/);
  assert.match(tauriSource, /validate_local_file_path/);
  assert.match(tauriMainSource, /Some\("usage-hook"\)/);
  assert.match(tauriMainSource, /record_skill_usage_from_hook/);
});

test('history page combines skill usage and operation logs', () => {
  assert.match(appSource, /function HistoryPage/);
  assert.match(appSource, /invoke\('list_history',\s*\{ request: \{ limit: 200 \} \}\)/);
  assert.match(appSource, /page === 'history'/);
  assert.match(appSource, /function normalizeHistory/);
  assert.match(appSource, /skillUsageCount/);
  assert.match(appSource, /operationCount/);
  assert.match(appSource, /entry\.kind === 'skill_usage'/);
  assert.match(appSource, /const rowSubtitle = historyRowSubtitle\(entry, isUsage\);/);
  assert.match(appSource, /function historyRowSubtitle\(entry, isUsage\)/);
  assert.match(appSource, /const defaultOperationSubtitle = entry\.operationType && entry\.actor/);
  assert.match(appSource, /const groupedEntries = groupHistoryEntriesByDay\(filteredEntries\)/);
  assert.match(appSource, /function groupHistoryEntriesByDay/);
  assert.match(appSource, /className="historyDayBlock"/);
  assert.match(appSource, /function HistoryRow/);
  assert.match(appSource, /className="historyRowTimestamp"/);
  assert.match(appSource, /className="historyRowTimeRail"/);
  assert.match(
    appSource,
    /<div className="historyRowTimeRail">[\s\S]*?<\/div>\s*<div className="historyRowTitle">/
  );
  assert.doesNotMatch(appSource, /timestampDate/);
  assert.doesNotMatch(appSource, /className="historyRowMarker"/);
  assert.match(appSource, /className="historyRowPrompt"/);
  assert.match(appSource, /rowSubtitle \? <p>\{rowSubtitle\}<\/p> : null/);
  assert.match(appSource, /entry\.promptExcerpt/);
  assert.match(tauriSource, /async fn list_history/);
  assert.match(tauriSource, /skillbox_core::list_history/);
  assert.match(css, /\.historyTimeline\s*\{/);
  assert.match(css, /\.historyDayBlock\s*\{/);
  assert.match(css, /\.historyRow\s*\{/);
  assert.match(css, /\.historyRow\s*\{[^}]*row-gap:\s*7px;/s);
  assert.match(css, /\.historyRowPrompt\s*\{/);
  assert.match(mainSource, /import '\.\/colors\.css';\s*import '\.\/styles\.css';/);
  assert.match(colorsCss, /--skillbox-prompt-bg:\s*#ecfdf5;/);
  assert.match(css, /\.historyRowPrompt\s*\{[^}]*background:\s*var\(--skillbox-prompt-bg\);/s);
  assert.match(css, /\.historyRowTimeRail\s*\{[^}]*grid-row:\s*1;/s);
  assert.match(css, /\.historyRowTitle\s*\{[^}]*grid-row:\s*1;/s);
  assert.match(css, /\.historyRowMain\s*\{[^}]*grid-column:\s*2;/s);
  assert.match(css, /\.historyRowTimestamp\s*\{[^}]*padding:\s*0 0 0 8px;/s);
  assert.match(css, /\.historyRowTimestamp strong\s*\{[^}]*line-height:\s*1\.25;/s);
  assert.doesNotMatch(css, /\.historyRowTimestamp span\s*\{/);
  assert.doesNotMatch(css, /\.historyRowMarker\s*\{/);
});

test('desktop startup reports run errors without expect panic', () => {
  assert.doesNotMatch(tauriSource, /\.expect\("failed to run SkillBox"\)/);
  assert.match(tauriSource, /eprintln!\("failed to run SkillBox: \{error\}"\)/);
});

test('remote skill async operations show loading and no-change states', () => {
  assert.match(appSource, /remoteContextLoading/);
  assert.match(appSource, /Loading remote details/);
  assert.match(appSource, /Loading diff/);
  assert.match(appSource, /No file changes in this skill/);
  assert.match(appSource, /inlineSpinner/);
});

test('skill detail modal uses a two-column workbench layout', () => {
  assert.match(appSource, /className="skillDetailBodyGrid"/);
  assert.match(appSource, /className="skillDetailMetaColumn"/);
  assert.match(appSource, /className="skillDetailControlRail"/);
  assert.match(appSource, /className="skillDetailVersionHistory"/);
  assert.match(css, /\.skillDetailDialog\s*\{[^}]*width:\s*min\(920px,\s*calc\(100vw - 48px\)\);/s);
  assert.match(css, /\.skillDetailBodyGrid\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)\s+minmax\(280px,\s*320px\);/s);
  assert.match(css, /\.skillDetailBodyGrid\s*\{[^}]*align-items:\s*start;/s);
  assert.match(css, /\.skillDetailBodyGrid\s*\{[^}]*overflow-x:\s*hidden;/s);
  assert.match(css, /\.skillDetailBodyGrid\s*\{[^}]*overflow-y:\s*auto;/s);
  assert.match(css, /\.skillDetailControlRail\s*\{[^}]*min-width:\s*0;/s);
  assert.match(css, /\.skillDetailControlRail\s*\{[^}]*align-self:\s*stretch;/s);
  assert.match(css, /\.skillDetailControlRail\s*\{[^}]*border-left:\s*1px solid #e2e8f0;/s);
  assert.match(css, /\.skillDetailControlRail\s*\{[^}]*position:\s*sticky;/s);
  assert.match(css, /\.remoteVersionSummary span\s*\{[^}]*white-space:\s*nowrap;/s);
  assert.match(css, /\.remoteVersionSummary span\s*\{[^}]*text-overflow:\s*ellipsis;/s);
  assert.match(css, /\.remoteVersionRow small\s*\{[^}]*white-space:\s*nowrap;/s);
  assert.match(css, /\.remoteVersionRow small\s*\{[^}]*text-overflow:\s*ellipsis;/s);
});

test('desktop preview defaults to hidden skillbox managed root', () => {
  assert.match(appSource, /root:\s*'~\/\.skillbox'/);
  assert.match(appSource, /userSkillsRoot:\s*'~\/\.skillbox\/user-skills'/);
  assert.match(appSource, /remoteSkillsRoot:\s*'~\/\.skillbox\/remote-skills'/);
  assert.match(appSource, /databasePath:\s*'~\/\.skillbox\/skillbox\.sqlite'/);
  assert.match(appSource, /userSkillsGit\.repoPath \|\| '~\/\.skillbox\/user-skills'/);
});

test('skill detail metadata starts with deploy workspace', () => {
  assert.match(appSource, /className="skillDetailMetaColumn"[\s\S]*aria-label="Deploy workspace"[\s\S]*<RemoteVersionHistoryPanel/);
  assert.match(appSource, /<span>Workspace deployment<\/span>[\s\S]*<button className="button secondary compactAction" type="button" onClick=\{onOpenDeployDialog\}/);
  assert.match(appSource, /className="skillDetailDeployMetric"[\s\S]*\{skill\.installedAgents\.length \|\| 0\}/);
  assert.match(appSource, /<strong>Active workspaces<\/strong>/);
  assert.match(appSource, /className="skillDetailUsageSummary"[\s\S]*\{skill\.usageCount \|\| 0\}[\s\S]*<strong>Usage<\/strong>/);
  assert.match(appSource, /labelPrefix="Deploy workspaces"/);
  assert.match(css, /\.skillDetailDeploySurface\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto;/s);
  assert.match(css, /\.skillDetailDeployMetric\s*\{/);
});

test('skill cards show usage directly under the skill name', () => {
  const skillCardStart = appSource.indexOf('function SkillCard');
  const skillCardEnd = appSource.indexOf('function AgentIconStack', skillCardStart);
  const skillCardSource = appSource.slice(skillCardStart, skillCardEnd);

  assert.ok(skillCardStart > 0);
  assert.ok(skillCardEnd > skillCardStart);
  assert.match(skillCardSource, /className="skillCardTitleText"[\s\S]*<strong>\{skill\.name\}<\/strong>[\s\S]*className="skillCardUsage"[\s\S]*\{skill\.usageCount \|\| 0\} calls/);
  assert.ok(
    skillCardSource.indexOf('<strong>{skill.name}</strong>') <
      skillCardSource.indexOf('className="skillCardUsage"')
  );
  assert.match(css, /\.skillCardTitleText\s*\{/);
  assert.match(css, /\.skillCardUsage\s*\{/);
});

test('skill cards use a shorter fixed card rhythm with aligned metadata rows', () => {
  const cardRule = css.match(/\.skillCard\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const hitAreaRule = css.match(/\.skillCardHitArea\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const tagRule = css.match(/\.skillCardTags\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(cardRule, /height:\s*216px;/);
  assert.match(hitAreaRule, /height:\s*100%;/);
  assert.match(hitAreaRule, /min-height:\s*216px;/);
  assert.match(hitAreaRule, /grid-template-rows:\s*auto minmax\(42px,\s*1fr\) 26px auto;/);
  assert.match(tagRule, /min-height:\s*26px;/);
});

test('skill card status and favorite action share one aligned header row', () => {
  const skillCardStart = appSource.indexOf('function SkillCard');
  const skillCardEnd = appSource.indexOf('function AgentIconStack', skillCardStart);
  const skillCardSource = appSource.slice(skillCardStart, skillCardEnd);
  const actionsRule = css.match(/\.skillCardHeaderActions\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const actionsBadgeRule = css.match(/\.skillCardHeaderActions \.badge\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const favoriteRule = css.match(/\.skillFavoriteButton\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(skillCardSource, /className="skillCardHeaderActions"[\s\S]*<Badge tone=\{skill\.statusTone\}>\{skill\.statusLabel\}<\/Badge>[\s\S]*className=\{skill\.isFavorite \? 'skillFavoriteButton active' : 'skillFavoriteButton'\}/);
  assert.match(actionsRule, /display:\s*inline-flex;/);
  assert.match(actionsRule, /align-items:\s*center;/);
  assert.match(actionsRule, /top:\s*20px;/);
  assert.match(actionsBadgeRule, /height:\s*32px;/);
  assert.match(favoriteRule, /width:\s*32px;/);
  assert.match(favoriteRule, /height:\s*32px;/);
  assert.doesNotMatch(favoriteRule, /position:\s*absolute;/);
});

test('deploy workspace dialog includes checked rows and unlink confirmation warning', () => {
  assert.match(appSource, /function DeployWorkspaceDialog/);
  assert.match(appSource, /workspaceDeployRequiresConfirmation\(changes\)/);
  assert.match(appSource, /confirmUndeploy/);
  assert.match(appSource, /Unchecked deployed workspaces will be unlinked/);
  assert.match(appSource, /aria-label=\{`Deploy \$\{skill\.name\} to workspace/);
  assert.match(css, /\.deployWorkspaceDialog\s*\{/);
  assert.match(css, /\.deployWorkspaceWarning\s*\{[^}]*background:\s*#fff7ed;/s);
});

test('installed workspace icons use immediate custom tooltips instead of native title delay', () => {
  assert.match(appSource, /data-tooltip=\{agent\.label\}/);
  assert.match(appSource, /aria-label=\{agent\.label\}/);
  assert.doesNotMatch(appSource, /className="skillAgentIcons" aria-label=\{label\} title=\{label\}/);
  assert.match(css, /\.skillAgentIcon\[data-tooltip\]::after\s*\{/);
  assert.match(css, /\.skillAgentIcon\[data-tooltip\]:hover::after,\s*\.skillAgentIcon\[data-tooltip\]:focus-visible::after\s*\{/);
  assert.doesNotMatch(css, /\.skillAgentIcon\[data-tooltip\]::after\s*\{[^}]*transition-delay:\s*[1-9]/s);
});

test('skill detail tags live inside controls rail', () => {
  assert.match(appSource, /<aside className="skillDetailControlRail" aria-label="Skill controls">[\s\S]*aria-label="Skill tags"[\s\S]*<RemoteSkillControlPanel/);
  assert.match(appSource, /<div className="skillDetailRailHeader">[\s\S]*<span>Controls<\/span>[\s\S]*<section className="skillDetailControlSection skillDetailTagsControl"/);
  assert.match(css, /\.skillDetailTagsControl \+ \.remoteSkillPanel,\s*\.skillDetailTagsControl \+ \.userSkillPanel\s*\{[^}]*border-top:\s*1px solid #eef2f7;/s);
});

test('remote update actions live in the detail control rail', () => {
  assert.match(appSource, /<RemoteSkillControlPanel[\s\S]*onCheckUpdates=\{onCheckUpdates\}/);
  assert.match(appSource, /className="skillDetailControlRail"[\s\S]*<RemoteSkillControlPanel/);
  assert.match(appSource, /const sourceLinked = Boolean\(remoteUpdate && remoteUpdate\.state !== 'no_source'\);/);
  assert.match(appSource, /const showReviewUpdate = remoteUpdate\?\.updateAvailable === true;/);
  assert.match(appSource, /const updateSectionLabel = showReviewUpdate \? 'Ready to review' : updateLabel;/);
  assert.match(appSource, /showReviewUpdate\s*\?\s*'Version change'/);
  assert.match(appSource, /showReviewUpdate && \/update available\/i\.test\(remoteUpdate\?\.message \|\| ''\)/);
  assert.match(appSource, /\{updateMessage \? <small>\{updateMessage\}<\/small> : null\}/);
  assert.match(appSource, /\{showReviewUpdate \? \(\s*<button\s+className="button primary"[\s\S]*Review update/);
  assert.match(appSource, /\{sourceLinked \? 'Rebind source' : 'Bind source'\}/);
  assert.doesNotMatch(appSource, /disabled=\{!remoteUpdate\?\.updateAvailable\}[\s\S]*Review update/);
  assert.doesNotMatch(appSource, /<footer className="skillDetailActions">[\s\S]*Check update/);
  assert.doesNotMatch(appSource, /<footer className="skillDetailActions">[\s\S]*onCheckUpdates/);
});

test('skill detail title exposes current remote source as a left-side action', () => {
  assert.match(appSource, /ExternalLink,/);
  assert.match(appSource, /sourceUrl=\{selectedRemoteUpdate\?\.sourceUrl \|\| ''\}/);
  assert.match(appSource, /onOpenSourceUrl=\{openRemoteSourceUrl\}/);
  assert.match(appSource, /async function openRemoteSourceUrl\(sourceUrl\)/);
  assert.match(appSource, /invoke\('open_external_url',\s*\{ url \}\)/);
  assert.match(appSource, /window\.open\(url,\s*'_blank',\s*'noopener,noreferrer'\)/);
  assert.match(appSource, /<div className="skillDetailTitleRow">[\s\S]*<h2 id="skill-detail-title">\{skill\.name\}<\/h2>[\s\S]*\{sourceUrl \? \(/);
  assert.match(appSource, /aria-label=\{`Open \$\{skill\.name\} source`\}/);
  assert.match(css, /\.skillDetailTitleRow\s*\{[^}]*display:\s*flex;/s);
  assert.doesNotMatch(css, /\.skillDetailSourceButton\s*\{[^}]*height:\s*32px;/s);
  assert.doesNotMatch(appSource, /<div className="skillDetailHeaderActions">[\s\S]*skillDetailSourceButton/);
});

test('skill detail title exposes local folder before remote source', () => {
  assert.match(appSource, /FolderOpen,/);
  assert.match(appSource, /onOpenLocalFolder=\{openLocalSkillFolder\}/);
  assert.match(appSource, /async function openLocalSkillFolder\(skill\)/);
  assert.match(appSource, /invoke\('open_local_path',\s*\{ path: folderPath \}\)/);
  assert.match(appSource, /aria-label=\{`Open \$\{skill\.name\} local folder`\}/);
  assert.match(
    appSource,
    /<div className="skillDetailTitleRow">[\s\S]*<h2 id="skill-detail-title">\{skill\.name\}<\/h2>[\s\S]*Folder[\s\S]*\{sourceUrl \? \(/
  );
});

test('skill detail favorite action lives in the header actions', () => {
  assert.match(appSource, /<div className="skillDetailHeaderActions">[\s\S]*className=\{skill\.isFavorite \? 'detailFavoriteButton active' : 'detailFavoriteButton'\}/);
  assert.match(appSource, /<div className="skillDetailHeaderActions">[\s\S]*onClick=\{\(\) => onToggleFavorite\(skill\.name\)\}/);
  assert.doesNotMatch(appSource, /<footer className="skillDetailActions">[\s\S]*detailFavoriteButton/);
});

test('button heights use shared global sizing tokens', () => {
  assert.match(css, /--button-height:\s*38px;/);
  assert.match(css, /\.button\s*\{[^}]*height:\s*var\(--button-height\);/s);
  assert.match(css, /\.iconButton\s*\{[^}]*width:\s*var\(--button-height\);[^}]*height:\s*var\(--button-height\);/s);
  assert.match(css, /\.detailFavoriteButton\s*\{[^}]*height:\s*var\(--button-height\);/s);
  assert.doesNotMatch(css, /\.skillDetailSourceButton\s*\{[^}]*height:\s*32px;/s);
  assert.doesNotMatch(css, /\.compactAction\s*\{[^}]*height:\s*32px;/s);
});

test('remote version list highlights the current version', () => {
  assert.match(appSource, /updatedAt:\s*version\.updatedAt \|\| version\.updated_at \|\| ''/);
  assert.match(appSource, /message:\s*version\.message \|\| ''/);
  assert.match(appSource, /const versionMeta = \[\s*version\.isCurrent \? 'Current' : version\.kind,\s*version\.message,\s*version\.updatedAt \? `Updated \$\{formatOperationTimestamp\(version\.updatedAt\)\}` : ''\s*\]/);
  assert.match(appSource, /<small>\{versionMeta\}<\/small>/);
  assert.match(appSource, /remoteVersionRow\$\{version\.isCurrent \? ' current' : ''\}/);
  assert.match(appSource, /aria-current=\{version\.isCurrent \? 'true' : undefined\}/);
  assert.match(appSource, /\{version\.isCurrent \? \(\s*<span className="button secondary remoteVersionCurrentBadge">Active<\/span>/);
  assert.match(css, /\.remoteVersionRow\s*\{[^}]*align-items:\s*start;/s);
  assert.match(css, /\.remoteVersionRow \.button\s*\{[^}]*align-self:\s*center;/s);
  assert.match(css, /\.remoteVersionRow \.button\s*\{[^}]*min-width:\s*88px;/s);
  assert.match(css, /\.remoteVersionRow\.current\s*\{[^}]*background:\s*#f7fff9;/s);
  assert.match(css, /\.remoteVersionRow\.current\s*\{[^}]*box-shadow:\s*inset 3px 0 0 #22c55e;/s);
  assert.match(css, /\.remoteVersionRow \.remoteVersionCurrentBadge\s*\{[^}]*display:\s*inline-flex;/s);
  assert.match(css, /\.remoteVersionRow \.remoteVersionCurrentBadge\s*\{[^}]*align-items:\s*center;/s);
  assert.match(css, /\.remoteVersionRow \.remoteVersionCurrentBadge\s*\{[^}]*justify-content:\s*center;/s);
  assert.match(css, /\.remoteVersionRow \.remoteVersionCurrentBadge\s*\{[^}]*color:\s*#166534;/s);
  assert.match(css, /\.remoteVersionRow \.remoteVersionCurrentBadge\s*\{[^}]*pointer-events:\s*none;/s);
});

test('version history shows the latest three rows before expanding older versions', () => {
  assert.match(appSource, /const VERSION_HISTORY_PREVIEW_COUNT = 3;/);
  assert.match(appSource, /const visibleVersions = expanded \|\| !hasHiddenVersions\s*\?\s*versionRows\s*:\s*versionRows\.slice\(0,\s*VERSION_HISTORY_PREVIEW_COUNT\);/);
  assert.match(appSource, /hiddenVersionCount = Math\.max\(0,\s*versionRows\.length - VERSION_HISTORY_PREVIEW_COUNT\)/);
  assert.match(appSource, /setExpanded\(\(current\) => !current\)/);
  assert.match(appSource, /Show \$\{hiddenVersionCount\} more/);
  assert.match(css, /\.remoteVersionToggle\s*\{/);
});

test('skill detail layout keeps deployment metadata before controls on narrow screens', () => {
  assert.match(css, /@media \(max-width:\s*920px\)\s*\{[\s\S]*\.skillDetailBodyGrid\s*\{[^}]*grid-template-columns:\s*1fr;/s);
  assert.match(css, /@media \(max-width:\s*920px\)\s*\{[\s\S]*\.skillDetailMetaColumn\s*\{[^}]*order:\s*1;/s);
  assert.match(css, /@media \(max-width:\s*920px\)\s*\{[\s\S]*\.skillDetailControlRail\s*\{[^}]*order:\s*2;/s);
});

test('remote version history stays in the metadata column before the log', () => {
  const versionHistoryRules = [...css.matchAll(/\.skillDetailVersionHistory\s*\{(?<body>[^}]*)\}/gs)]
    .map((match) => match.groups.body)
    .join('\n');

  assert.match(appSource, /className="skillDetailMetaColumn"[\s\S]*<RemoteVersionHistoryPanel[\s\S]*<OperationHistoryPanel operations=\{operations\} \/>[\s\S]*<\/div>\s*<aside className="skillDetailControlRail"/);
  assert.doesNotMatch(css, /\.skillDetailVersionHistory\s*\{[^}]*grid-column:\s*1 \/ -1;/s);
  assert.doesNotMatch(versionHistoryRules, /(^|[;\s])order\s*:/);
});

test('local skill detail renders version history from the user skills git repo', () => {
  assert.match(tauriSource, /fn list_user_skill_versions\(skill_name:\s*String\)/);
  assert.match(appSource, /invoke\('list_user_skill_versions',\s*\{\s*skillName\s*\}\)/);
  assert.match(appSource, /function UserSkillVersionHistoryPanel/);
  assert.match(appSource, /skill\.type === 'user' \? \(\s*<UserSkillVersionHistoryPanel/s);
  assert.match(appSource, /aria-label="User skill version history"/);
});

test('remote operation history is collapsed by default', () => {
  assert.match(appSource, /<details className="operationHistoryPanel" aria-label="Operation history">/);
  assert.match(appSource, /<summary className="operationHistorySummary">/);
  assert.match(appSource, /\{operations\.length\} events/);
  assert.doesNotMatch(appSource, /<div className="operationHistoryPanel" aria-label="Operation history">/);
});

test('remote operation history rows include timestamps', () => {
  assert.match(appSource, /formatOperationTimestamp\(operation\.finishedAt \|\| operation\.startedAt\)/);
  assert.match(appSource, /<time dateTime=\{operation\.finishedAt \|\| operation\.startedAt\}>/);
  assert.match(css, /\.operationHistoryRow time/);
});

test('manual remote source submit verifies and binds without a separate preview action', () => {
  assert.match(appSource, /async function verifyAndBindRemoteSource\(event\)/);
  assert.match(appSource, /event\?\.preventDefault\?\.\(\)/);
  assert.match(appSource, /await loadRemoteSourceBindingPreview\(skillName,\s*trimmedSourceUrl\)/);
  assert.match(appSource, /preview\.validation === 'mismatch'/);
  assert.match(appSource, /source_url:\s*verifiedSourceUrl/);
  assert.match(appSource, /onBind=\{verifyAndBindRemoteSource\}/);
  assert.match(appSource, /<form className="remoteImportForm" onSubmit=\{onBind\}>/);
  assert.match(appSource, /Verify and Bind Source/);
  assert.doesNotMatch(appSource, />\s*Preview\s*<\/button>/);
  assert.doesNotMatch(appSource, /onPreview=\{previewRemoteSourceBinding\}/);
});

test('remote source candidates use view and bind actions instead of inline preview', () => {
  assert.match(appSource, /onViewCandidate\(candidate\)/);
  assert.match(appSource, /onBindCandidate\(candidate\)/);
  assert.match(appSource, /Suggested Claude Marketplace matches/);
  assert.match(appSource, />\s*View\s*<\/button>/);
  assert.match(appSource, />\s*Bind\s*<\/button>/);
  assert.doesNotMatch(appSource, /onPreviewCandidate\(candidate\)/);
});

test('remote source candidate bind confirmation checks before final binding', () => {
  assert.match(appSource, /function RemoteSourceCandidateBindDialog/);
  assert.match(appSource, /Checking source/);
  assert.match(appSource, /Confirm bind/);
  assert.match(appSource, /Binding\.\.\./);
  assert.match(appSource, /disabled=\{!canConfirm\}/);
});

test('remote source candidate view opens through the desktop bridge with a browser fallback', () => {
  assert.match(appSource, /async function viewRemoteSourceCandidate\(candidate\)/);
  assert.match(appSource, /invoke\('open_external_url'/);
  assert.match(appSource, /window\.open\(sourceUrl,\s*'_blank',\s*'noopener,noreferrer'\)/);
});
