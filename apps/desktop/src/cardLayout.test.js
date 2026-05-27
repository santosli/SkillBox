import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const css = await readFile(new URL('./styles.css', import.meta.url), 'utf8');
const appSource = await readFile(new URL('./App.jsx', import.meta.url), 'utf8');
const tauriSource = await readFile(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');

test('dashboard and workspace cards share a fixed auto-wrapping grid width', () => {
  const sharedGridRule = css.match(/\.skillCardGrid,\s*\.workspaceCardGrid\s*\{(?<body>[^}]*)\}/s)
    ?.groups.body || '';

  assert.match(css, /--dashboard-card-width:\s*360px;/);
  assert.match(css, /--dashboard-card-track:\s*minmax\(var\(--dashboard-card-width\),\s*var\(--dashboard-card-width\)\);/);
  assert.match(
    sharedGridRule,
    /grid-template-columns:\s*repeat\(auto-fill,\s*var\(--dashboard-card-track\)\);/
  );
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
  assert.match(sharedIconRule, /border:\s*1px solid #e5e7eb;/);
  assert.match(sharedSvgRule, /width:\s*15px;/);
  assert.match(sharedSvgRule, /height:\s*15px;/);
  assert.doesNotMatch(css, /\.sidebarFooter button svg\s*\{[^}]*width:\s*22px;/s);
});

test('dashboard actions stay in one equal segmented row', () => {
  const contentRule = css.match(/\.content\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const controlRowRule = css.match(/\.dashboardControlRow\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const typeTabsRule = css.match(/\.dashboardTypeTabs\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
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
  assert.match(actionGroupRule, /width:\s*330px;/);
  assert.match(actionGroupRule, /grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\);/);
  assert.match(css, /\.dashboardActionGroup\.previewing \.dashboardActionIndicator\s*\{[^}]*opacity:\s*1;/s);
  assert.match(indicatorRule, /opacity:\s*0;/);
  assert.match(indicatorRule, /transform:\s*translateX\(calc\(var\(--dashboard-action-index,\s*0\) \* 100%\)\);/);
  assert.match(indicatorRule, /transform 280ms cubic-bezier\(0\.2,\s*0\.8,\s*0\.2,\s*1\);/);
  assert.match(appSource, /label:\s*'Refresh'/);
  assert.match(appSource, /label:\s*'Import'/);
  assert.match(appSource, /label:\s*'Install'/);
  assert.match(appSource, /onMouseEnter=\{\(\) => setPreviewAction\(action\.id\)\}/);
  assert.match(appSource, /onBlur=\{\(event\) =>/);
  assert.match(appSource, /setPreviewAction\(null\);/);
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

test('remote update preview command runs off the command handler', () => {
  const previewCommandStart = tauriSource.indexOf('async fn preview_remote_version_change');
  const nextCommandStart = tauriSource.indexOf('#[tauri::command]', previewCommandStart + 1);
  const previewCommand = tauriSource.slice(previewCommandStart, nextCommandStart);

  assert.ok(previewCommandStart > 0);
  assert.match(previewCommand, /tauri::async_runtime::spawn_blocking/);
});

test('remote skill async operations show loading and no-change states', () => {
  assert.match(appSource, /remoteContextLoading/);
  assert.match(appSource, /Loading remote details/);
  assert.match(appSource, /Loading diff/);
  assert.match(appSource, /No file changes in this skill/);
  assert.match(appSource, /inlineSpinner/);
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
