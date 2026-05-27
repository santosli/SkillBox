import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const css = await readFile(new URL('./styles.css', import.meta.url), 'utf8');
const appSource = await readFile(new URL('./App.jsx', import.meta.url), 'utf8');

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
