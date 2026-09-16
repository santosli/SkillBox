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
  './components/rankings.jsx',
  './components/settings.jsx',
  './components/importReview.jsx',
  './components/skillDetail.jsx',
  './components/remoteSkills.jsx',
  './components/userSkillsSync.jsx',
  './skills.js',
  './historyEntries.js',
  './usageRankings.js',
  './usageHooks.js',
  './workspaces.js',
  './appUpdates.js',
  './preferences.js',
  './previewData.js',
  './importFlow.js',
  './workspaceDirectoryPicker.js',
  './appActionsA.js',
  './appActionsB.js'
];
const appSource = (
  await Promise.all(
    appSourcePaths.map((path) => readFile(new URL(path, import.meta.url), 'utf8'))
  )
).join('\n');
const appComponentSource = (
  await Promise.all(
    ['./App.jsx', './appActionsA.js', './appActionsB.js'].map((path) =>
      readFile(new URL(path, import.meta.url), 'utf8')
    )
  )
).join('\n');
const mainSource = await readFile(new URL('./main.jsx', import.meta.url), 'utf8');
const tauriSource = await readFile(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const tauriMainSource = await readFile(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');
const tauriCargo = await readFile(new URL('../src-tauri/Cargo.toml', import.meta.url), 'utf8');
const tauriMainCapability = JSON.parse(
  await readFile(new URL('../src-tauri/capabilities/main.json', import.meta.url), 'utf8')
);
const desktopPackage = JSON.parse(
  await readFile(new URL('../package.json', import.meta.url), 'utf8')
);

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

test('sidebar brand does not render a subtitle', () => {
  const brandRule = css.match(/\.brand\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const brandTextRule = css.match(/\.brandName\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const brandTitleRule = css.match(/\.brand strong\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(
    appSource,
    /const APP_DISPLAY_NAME = import\.meta\.env\.DEV && !publicPreview \? 'SkillBox Dev' : 'SkillBox';/
  );
  assert.match(appSource, /<strong>\{APP_DISPLAY_NAME\}<\/strong>/);
  assert.doesNotMatch(appSource, /Local skill manager/);
  assert.doesNotMatch(css, /\.brand span/);
  assert.match(brandRule, /gap:\s*9px;/);
  assert.match(brandTextRule, /min-height:\s*36px;/);
  assert.match(brandTextRule, /align-items:\s*center;/);
  assert.match(brandTitleRule, /font-size:\s*21px;/);
  assert.match(brandTitleRule, /line-height:\s*36px;/);
});

test('sidebar exposes a disabled-while-installing update action only when an update is available', () => {
  const buttonSource =
    appComponentSource.match(
      /\{appUpdate\.available \? \([\s\S]*?className="sidebarUpdateButton"[\s\S]*?\) : null\}/
    )?.[0] || '';
  const buttonRule =
    css.match(/\.sidebarUpdateButton\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(buttonSource, /Update SkillBox to version/);
  assert.match(buttonSource, /Install SkillBox v/);
  assert.match(buttonSource, /appUpdate\.state === 'installing'/);
  assert.match(buttonSource, /appUpdateInstallBlocked/);
  assert.match(buttonSource, /Updating…/);
  assert.match(buttonSource, /onClick=\{requestAppUpdateInstall\}/);
  assert.doesNotMatch(buttonSource, /onClick=\{installAppUpdate\}/);
  assert.match(buttonRule, /height:\s*24px;/);
  assert.match(buttonRule, /padding:\s*0 6px;/);
  assert.match(buttonRule, /background:\s*var\(--skillbox-blue\);/);
  assert.match(buttonRule, /color:\s*var\(--skillbox-surface\);/);
  assert.doesNotMatch(buttonRule, /margin-left:\s*auto;/);
  assert.match(appComponentSource, /if \(appUpdateInstallBlocked\)/);
  assert.match(appComponentSource, /Development preview only\. Packaged release builds/);
  assert.match(appComponentSource, /appUpdateDialog\.open/);
  assert.match(appComponentSource, /onConfirm=\{installAppUpdate\}/);
  assert.match(appSource, /export function AppUpdateConfirmDialog/);
  assert.match(appSource, /Install SkillBox update\?/);
  assert.match(appSource, /isDisabled \|\| installBlocked/);
  assert.match(appComponentSource, /import\.meta\.env\.DEV[\s\S]*previewAppUpdateStatus/);
});

test('dashboard and settings use the shared page title row template', () => {
  const dashboardSource = appSource.match(/export function Dashboard[\s\S]*?function DashboardActionGroup/)?.[0] || '';
  const settingsPageSource = appSource.match(/export function SettingsPage[\s\S]*?function SettingsRail/)?.[0] || '';
  const commonSource = appSource.match(/export function PageTitleRow[\s\S]*?export function NavButton/)?.[0] || '';
  const pageRule = css.match(/\.settingsPage\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const pageTitleRowRule = css.match(/\.pageTitleRow\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const pageTitleGroupRule = css.match(/\.pageTitleGroup\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const pageTitleHeadingRule = css.match(/\.pageTitleGroup h1\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const pageTitlePillRule = css.match(/\.pageTitlePill\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const workbenchRule = css.match(/\.settingsWorkbench\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(commonSource, /export function PageTitleRow\(\{ actions, count, subtitle, title \}\)/);
  assert.match(commonSource, /className="pageTitleRow"/);
  assert.match(commonSource, /className="pageTitleGroup"/);
  assert.match(commonSource, /className="pageTitleHeading"/);
  assert.match(commonSource, /className="pageTitlePill"/);
  assert.match(commonSource, /className="pageTitleSubtitle"/);
  assert.match(dashboardSource, /<PageTitleRow[\s\S]*title="Skills"[\s\S]*count=\{filtered\.length\}[\s\S]*actions=/);
  assert.match(settingsPageSource, /<PageTitleRow title="Settings" \/>/);
  assert.doesNotMatch(settingsPageSource, /<PageHeader/);
  assert.doesNotMatch(settingsPageSource, /settingsPageHeader/);
  assert.doesNotMatch(settingsPageSource, /eyebrow="Settings"/);
  assert.doesNotMatch(settingsPageSource, /subtitle="Storage, sync, updates, and hooks\."/);
  assert.doesNotMatch(css, /\.settingsPageHeader/);
  assert.doesNotMatch(css, /\.dashboardTitleRow/);
  assert.doesNotMatch(css, /\.dashboardTitleGroup/);
  assert.doesNotMatch(css, /\.dashboardCountPill/);
  assert.match(pageRule, /display:\s*grid;/);
  assert.match(pageRule, /max-width:\s*1220px;/);
  assert.match(pageRule, /gap:\s*18px;/);
  assert.match(pageTitleRowRule, /min-height:\s*60px;/);
  assert.match(pageTitleRowRule, /border-bottom:\s*1px solid var\(--skillbox-border\);/);
  assert.match(pageTitleRowRule, /padding-bottom:\s*12px;/);
  assert.match(pageTitleGroupRule, /gap:\s*12px;/);
  assert.match(pageTitleHeadingRule, /font-size:\s*28px;/);
  assert.match(pageTitleHeadingRule, /font-weight:\s*700;/);
  assert.match(pageTitlePillRule, /min-width:\s*40px;/);
  assert.match(pageTitlePillRule, /height:\s*32px;/);
  assert.match(workbenchRule, /width:\s*100%;/);
  assert.match(workbenchRule, /max-width:\s*1220px;/);
});

test('settings receives persistent inbound apply warnings and an explicit dismiss action', () => {
  assert.match(appComponentSource, /userSkillsInboundWarnings=\{userSkillsInboundWarnings\}/);
  assert.match(
    appComponentSource,
    /onDismissUserSkillsInboundWarnings=\{\(\) => setUserSkillsInboundWarnings\(\[\]\)\}/
  );
  assert.match(appSource, /<UserSkillsInboundApplyWarning/);
  assert.match(appSource, /warnings=\{userSkillsInboundWarnings\}/);
  assert.match(appSource, /onDismiss=\{onDismissInboundWarnings\}/);
});

test('tauri desktop bridge registers app update commands and pending state', () => {
  assert.match(tauriSource, /struct PendingAppUpdate/);
  assert.match(tauriSource, /fn app_update_disabled_response/);
  assert.match(tauriSource, /async fn check_app_update/);
  assert.match(tauriSource, /async fn install_app_update/);
  assert.match(tauriSource, /APP_UPDATE_CHECK_INTERVAL_SECONDS/);
  assert.match(tauriSource, /struct AppUpdateSessionCache/);
  assert.match(tauriSource, /cached_app_update_check/);
  assert.match(tauriSource, /cache_app_update_response/);
  assert.match(tauriSource, /Unable to persist the app update check cache/);
  assert.match(tauriSource, /let cached_pending_update = \{[\s\S]*pending\.clone\(\)/);
  assert.match(tauriSource, /tauri_plugin_updater::Builder::new\(\)\.build\(\)/);
  assert.match(tauriSource, /app\.manage\(PendingAppUpdate::default\(\)\)/);
  assert.match(tauriSource, /app\.manage\(AppUpdateSessionCache::default\(\)\)/);
});

test('tauri desktop grants only native directory open permission for workspace picking', () => {
  assert.equal(desktopPackage.dependencies['@tauri-apps/plugin-dialog'], '2.7.2');
  assert.match(tauriCargo, /tauri-plugin-dialog = "=2\.7\.2"/);
  assert.match(tauriSource, /\.plugin\(tauri_plugin_dialog::init\(\)\)/);
  assert.deepEqual(tauriMainCapability.windows, ['main']);
  assert.deepEqual(tauriMainCapability.permissions, ['core:default', 'dialog:allow-open']);
});

test('dashboard search shortcut and empty state provide recovery actions', () => {
  assert.match(appSource, /ref=\{searchInputRef\}/);
  assert.match(appSource, /\(event\.metaKey \|\| event\.ctrlKey\)/);
  assert.match(appSource, /event\.key\.toLowerCase\(\) === 'f'/);
  assert.match(appSource, /event\.preventDefault\(\);[\s\S]*searchInputRef\.current\?\.focus\(\);[\s\S]*searchInputRef\.current\?\.select\(\);/);
  assert.match(appSource, /window\.addEventListener\('keydown', focusDashboardSearch\)/);
  assert.match(appSource, /window\.removeEventListener\('keydown', focusDashboardSearch\)/);
  assert.match(appSource, />⌘F<\/span>/);
  assert.match(appSource, /hasActiveFilters \? \([\s\S]*Clear filters/);
  assert.match(appSource, /function clearDashboardFilters\(\)[\s\S]*setQuery\(''\);[\s\S]*setFilter\('all'\);[\s\S]*setDashboardTagFilter\('all'\);[\s\S]*setDashboardFavoritesOnly\(false\);/);
});

test('dashboard view controls expose labels and tooltips', () => {
  assert.match(appSource, /aria-label="Show card view"[\s\S]*title="Card view"/);
  assert.match(appSource, /aria-label="Show list view"[\s\S]*title="List view"/);
});

test('first-use dashboard explains safe setup before local changes', () => {
  const firstUseSource = appSource.match(/function FirstUseDashboard[\s\S]*?\n\}/)?.[0] || '';
  const panelRule = css.match(/\.firstUsePanel\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const stepRule = css.match(/\.firstUseStep\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(firstUseSource, /Set up SkillBox safely/);
  assert.match(firstUseSource, /does not change runtime folders until you review and\s+confirm an action/);
  assert.match(appSource, /ScanSearch/);
  assert.match(appSource, /ShieldCheck/);
  assert.match(appSource, /GitBranch/);
  assert.match(firstUseSource, /className="firstUseWorkflow"/);
  assert.match(firstUseSource, /className="firstUseFlowItem"/);
  assert.doesNotMatch(firstUseSource, /emptyGlyph/);
  assert.match(firstUseSource, /Scan workspaces/);
  assert.match(firstUseSource, /scan is read-only/);
  assert.match(firstUseSource, /Review imports/);
  assert.match(firstUseSource, /copies anything into[\s\S]*~\/\.skillbox/);
  assert.match(firstUseSource, /Deploy intentionally/);
  assert.match(firstUseSource, /linked only when you choose a workspace target/);
  assert.match(firstUseSource, /Read-only scan/);
  assert.match(firstUseSource, /Review before copy/);
  assert.match(firstUseSource, /No silent overwrite/);
  assert.match(firstUseSource, /onClick=\{onScan\}[\s\S]*Scan local skills/);
  assert.match(firstUseSource, /onClick=\{onInstall\}[\s\S]*Install from GitHub/);
  assert.doesNotMatch(firstUseSource, /scan writes/i);
  assert.doesNotMatch(firstUseSource, /scan changes/i);
  assert.match(css, /\.firstUseChecklist\s*\{/);
  assert.match(css, /\.firstUseSafetyStrip\s*\{/);
  assert.match(css, /\.firstUseWorkflow\s*\{/);
  assert.match(css, /\.firstUseStepIcon\s*\{/);
  assert.match(panelRule, /min-height:\s*360px;/);
  assert.match(panelRule, /padding:\s*42px;/);
  assert.match(stepRule, /border:\s*1px solid #edf2f7;/);
});

test('workspace skill tabs stay visible instead of collapsing into a scrollbar', () => {
  const workspaceSkillTabsRule = css.match(/\.workspaceSkillTabs\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const workspaceSkillTabButtonRule = css.match(/\.workspaceSkillTabs button\s*\{(?<body>[^}]*)\}/s)
    ?.groups.body || '';

  assert.match(workspaceSkillTabsRule, /display:\s*flex;/);
  assert.match(workspaceSkillTabsRule, /flex-wrap:\s*wrap;/);
  assert.match(workspaceSkillTabsRule, /width:\s*100%;/);
  assert.match(workspaceSkillTabsRule, /overflow:\s*visible;/);
  assert.match(workspaceSkillTabButtonRule, /flex:\s*0 0 auto;/);
  assert.doesNotMatch(workspaceSkillTabsRule, /width:\s*max-content;/);
  assert.doesNotMatch(workspaceSkillTabsRule, /overflow-x:\s*auto;/);
});

test('import candidate groups disclose locations and use radio variant selection', () => {
  const groupSource = appSource.match(
    /function CandidateGroupCard\(\{ group, onSelectVariant, onToggleSelected, onTypeChange \}\)\s*\{(?<body>[\s\S]*?)\n\}/
  )?.groups.body || '';
  const disclosureRule = css.match(/\.candidateLocationsDisclosure\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const locationRule = css.match(/\.candidateLocation\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const typeActionRule = css.match(/\.candidateTypeAction\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const requiredTypeActionRule = css.match(/\.candidateTypeAction\.required\s*\{(?<body>[^}]*)\}/s)
    ?.groups.body || '';
  const typeActionSwitchRule = css.match(/\.candidateTypeAction \.candidateTypeSwitch\s*\{(?<body>[^}]*)\}/s)
    ?.groups.body || '';
  const typeActionButtonRule = css.match(
    /\.candidateTypeAction \.candidateTypeSwitch button\s*\{(?<body>[^}]*)\}/s
  )?.groups.body || '';

  assert.match(groupSource, /aria-controls=\{disclosureId\}/);
  assert.match(groupSource, /aria-expanded=\{expanded\}/);
  assert.match(groupSource, /Found in \{locationCount\}/);
  assert.match(groupSource, /type="radio"/);
  assert.match(groupSource, /group\.variants\.length > 1/);
  assert.match(groupSource, /name=\{`\$\{group\.id\}-variant`\}/);
  assert.match(groupSource, /onSelectVariant\(group, variant\)/);
  assert.match(groupSource, /Source: \{compactPath\(location\.symlinkTargetPath \|\| location\.realPath\)\}/);
  assert.match(groupSource, /Mixed type suggestions/);
  assert.match(groupSource, /id=\{typeLabelId\}>Skill type</);
  assert.match(groupSource, /<strong>Required<\/strong>/);
  assert.match(groupSource, /Choose where SkillBox should manage this skill/);
  assert.match(groupSource, /aria-describedby=\{needsTypeChoice \? typeHelpId : undefined\}/);
  assert.match(groupSource, /aria-labelledby=\{typeLabelId\}/);
  assert.match(groupSource, /aria-required=\{needsTypeChoice\}/);
  assert.match(groupSource, /role="radiogroup"/);
  assert.match(groupSource, /role="radio"/);
  assert.match(groupSource, /aria-checked=\{selectedVariant\?\.selectedType === 'user'\}/);
  assert.match(groupSource, /aria-checked=\{selectedVariant\?\.selectedType === 'remote'\}/);
  assert.doesNotMatch(groupSource, /Choose User or Remote before importing this skill/);
  assert.match(groupSource, /selectedVariant\?\.selectedType === 'user'/);
  assert.match(groupSource, /selectedVariant\?\.selectedType === 'remote'/);
  assert.match(groupSource, /disabled=\{!canClassifyImportCandidateGroup\(group\)\}/);
  assert.match(disclosureRule, /display:\s*inline-flex;/);
  assert.match(locationRule, /grid-template-columns:\s*96px minmax\(0,\s*1fr\);/);
  assert.match(typeActionRule, /display:\s*grid;/);
  assert.match(typeActionRule, /width:\s*208px;/);
  assert.match(typeActionRule, /box-sizing:\s*border-box;/);
  assert.match(typeActionRule, /justify-self:\s*end;/);
  assert.match(requiredTypeActionRule, /border-color:\s*var\(--skillbox-amber-border\);/);
  assert.match(requiredTypeActionRule, /background:\s*var\(--skillbox-surface-orange\);/);
  assert.match(typeActionSwitchRule, /width:\s*100%;/);
  assert.match(typeActionButtonRule, /flex:\s*1 1 0;/);
});

test('collection review keeps child selection and collection type controls inside one expandable card', () => {
  const collectionSource = appSource.match(
    /function CollectionReviewCard\(\{(?<body>[\s\S]*?)\n\}\n\nfunction WorkspaceSkillTabs/
  )?.groups.body || '';
  const collectionCheckboxSource = appSource.match(
    /function CollectionSelectionCheckbox\(\{(?<body>[\s\S]*?)\n\}\n\nfunction CollectionReviewCard/
  )?.groups.body || '';
  const candidateReviewListSource = appSource.match(
    /function CandidateReviewList\(\{(?<body>[\s\S]*?)\n\}\n\nfunction CollectionSelectionCheckbox/
  )?.groups.body || '';
  const collectionRule = css.match(/\.collectionReviewCard\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const collectionHeaderRule = css.match(/\.collectionReviewHeader\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const childRule = css.match(/\.collectionChildRow\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const collectionActionsRule = css.match(/\.collectionReviewActions\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const collectionTypeRule = css.match(/\.collectionReviewTypeAction\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';
  const collectionSelectionRule = css.match(
    /\.collectionReviewSelectionAction\s*\{(?<body>[^}]*)\}/s
  )?.groups.body || '';
  const requiredCollectionTypeRule = css.match(
    /\.collectionReviewTypeAction\.required\s*\{(?<body>[^}]*)\}/s
  )?.groups.body || '';
  const narrowCollectionRules = css.match(
    /@media \(max-width: 920px\)\s*\{(?<body>[\s\S]*)\n\}/
  )?.groups.body || '';

  assert.match(collectionSource, /aria-controls={disclosureId}/);
  assert.match(collectionSource, /collection\.children\.map/);
  assert.match(collectionSource, /collectionSelectionState\(groups, selectionCollection\)/);
  assert.match(collectionSource, /collectionTypeChoiceState\(groups, selectionCollection\)/);
  assert.match(collectionSource, /<CollectionSelectionCheckbox/);
  assert.match(collectionSource, /onToggleCollectionSelected\(selectionCollection\)/);
  assert.match(collectionSource, /id=\{typeLabelId\}>Import as/);
  assert.match(collectionSource, /<strong>Required<\/strong>/);
  assert.match(collectionSource, /Choose one type for every pending skill in this collection/);
  assert.match(collectionSource, /aria-describedby=\{typeState\.required \? typeHelpId : undefined\}/);
  assert.match(collectionSource, /aria-labelledby=\{typeLabelId\}/);
  assert.match(collectionSource, /aria-required=\{typeState\.required\}/);
  assert.match(collectionSource, /role="radiogroup"/);
  assert.match(collectionSource, /role="radio"/);
  assert.match(collectionSource, /aria-checked=\{typeState\.selectedType === 'user'\}/);
  assert.match(collectionSource, /aria-checked=\{typeState\.selectedType === 'remote'\}/);
  assert.match(collectionSource, /onCollectionTypeChange\(selectionCollection, 'user'\)/);
  assert.match(collectionSource, /onCollectionTypeChange\(selectionCollection, 'remote'\)/);
  assert.match(collectionSource, /typeState\.actionableCount > 0/);
  assert.doesNotMatch(collectionSource, /No pending skills/);
  assert.match(collectionSource, /const selected = !typeState\.required/);
  assert.match(collectionSource, /disabled=\{!canSelect \|\| typeState\.required \|\| selectionDisabled \|\| child\.selectionLocked\}/);
  assert.match(appSource, /const selectableCount = importReviewSelectableGroups\(groups, collections\)\.length/);
  assert.match(appSource, /selectedImportCollectionRequests\(groups, collections\)/);
  assert.match(collectionCheckboxSource, /checkboxRef\.current\.indeterminate = indeterminate/);
  assert.match(collectionCheckboxSource, /aria-label=\{`Select all eligible skills in \$\{collectionName\}`\}/);
  assert.match(collectionCheckboxSource, /aria-describedby=\{ariaDescribedBy\}/);
  assert.match(collectionCheckboxSource, /disabled=\{disabled\}/);
  assert.match(candidateReviewListSource, /selectionCollection=\{collections\.find/);
  assert.match(candidateReviewListSource, /onToggleCollectionSelected/);
  assert.match(candidateReviewListSource, /onCollectionTypeChange/);
  assert.match(appSource, /updateImportCollectionType\(current\.candidates, collection, skillType\)/);
  assert.match(appComponentSource, /toggleImportReviewSelection\(current\.candidates, current\.collections\)/);
  assert.match(collectionSource, /onToggleSelected\(group\)/);
  assert.match(collectionSource, /collectionChildTypeState\(group, child\)/);
  assert.match(collectionSource, /collection\.sourceKind === 'installed_source'/);
  assert.doesNotMatch(collectionSource, /disabled=\{!canClassifyImportCandidateGroup\(group\)\}/);
  assert.doesNotMatch(collectionSource, /aria-label=\{`\$\{child\.name\} skill type`\}/);
  assert.doesNotMatch(collectionSource, /onTypeChange\(group/);
  assert.match(collectionSource, /readOnlyLabel \?/);
  assert.match(collectionSource, /relativePath/);
  assert.match(collectionRule, /min-width:\s*0;/);
  assert.match(collectionHeaderRule, /display:\s*grid;/);
  assert.match(collectionHeaderRule, /grid-template-columns:\s*minmax\(0,\s*1fr\) minmax\(470px,\s*auto\);/);
  assert.match(collectionHeaderRule, /align-items:\s*flex-start;/);
  assert.match(collectionActionsRule, /display:\s*grid;/);
  assert.match(collectionActionsRule, /width:\s*470px;/);
  assert.match(collectionActionsRule, /grid-template-columns:\s*248px minmax\(0,\s*210px\);/);
  assert.match(collectionActionsRule, /align-self:\s*start;/);
  assert.match(collectionTypeRule, /width:\s*248px;/);
  assert.match(collectionTypeRule, /box-sizing:\s*border-box;/);
  assert.match(collectionSelectionRule, /grid-column:\s*2;/);
  assert.match(collectionSelectionRule, /align-items:\s*flex-start;/);
  assert.match(narrowCollectionRules, /\.collectionReviewActions\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\);/s);
  assert.match(narrowCollectionRules, /\.collectionReviewSelectionAction\s*\{[^}]*grid-column:\s*1;/s);
  assert.match(requiredCollectionTypeRule, /border-color:\s*var\(--skillbox-amber-border\);/);
  assert.match(requiredCollectionTypeRule, /background:\s*var\(--skillbox-surface-orange\);/);
  assert.match(css, /\.candidateCheck input:indeterminate \+ span/);
  assert.match(childRule, /grid-template-columns:\s*28px minmax\(0,\s*1fr\) auto;/);
});

test('remote skill URL import previews GitHub skills before install', () => {
  const submitRemoteImport = appSource.match(
    /async function submitRemoteImport\(event\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(submitRemoteImport, /invoke\('preview_github_remote_skill_install',\s*\{\s*request:\s*\{/);
  assert.match(submitRemoteImport, /source_url:\s*value/);
  assert.match(submitRemoteImport, /target_root:\s*null/);
  assert.match(submitRemoteImport, /setRemoteInstallDialog\(/);
  assert.match(submitRemoteImport, /applyLabel:\s*'Install from GitHub'/);
  assert.doesNotMatch(submitRemoteImport, /invoke\('install_github_remote_skill'/);
  assert.doesNotMatch(submitRemoteImport, /invoke\('parse_github_url'/);
  assert.doesNotMatch(appSource, /Remote download\/import is not wired yet\./);
  assert.match(appSource, /Standalone repositories with a root SKILL\.md and skill directories are supported\./);
  assert.doesNotMatch(appSource, /Repository-root SKILL\.md files are not supported\./);
});

test('remote GitHub collection update commands run off the command handler', () => {
  for (const commandName of [
    'preview_github_skill_collection_update',
    'apply_github_skill_collection_update',
    'preview_github_skill_collection_rollback',
    'apply_github_skill_collection_rollback'
  ]) {
    const commandStart = tauriSource.indexOf(`async fn ${commandName}`);
    const nextCommandStart = tauriSource.indexOf('#[tauri::command]', commandStart + 1);
    const command = tauriSource.slice(commandStart, nextCommandStart);

    assert.ok(commandStart > 0, `${commandName} should be registered as a command`);
    assert.match(command, /tauri::async_runtime::spawn_blocking/);
  }
});

test('remote GitHub install commands run off the command handler', () => {
  for (const commandName of ['preview_github_remote_skill_install', 'install_github_remote_skill']) {
    const commandStart = tauriSource.indexOf(`async fn ${commandName}`);
    const nextCommandStart = tauriSource.indexOf('#[tauri::command]', commandStart + 1);
    const command = tauriSource.slice(commandStart, nextCommandStart);

    assert.ok(commandStart > 0, `${commandName} should be registered as a command`);
    assert.match(command, /tauri::async_runtime::spawn_blocking/);
    assert.match(tauriSource, new RegExp(`${commandName},`));
  }
});

test('dashboard startup loads cached remote update state without refreshing', () => {
  assert.match(appSource, /invoke\('cached_remote_skill_updates'\)/);
  assert.match(appSource, /setRemoteSkillUpdates\(cachedRemoteUpdates\)/);
  assert.match(appSource, /setLastStatusCheckedAt\(cachedRemoteUpdates\.checkedAt \|\| ''\)/);
});

test('dashboard refresh checks all remote skills while detail check targets one skill', () => {
  const refreshStatuses = appSource.match(
    /async function refreshSkillStatuses\(\{ automatic = false, skillName = '' \} = \{\}\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(refreshStatuses, /if \(skillName\) \{[\s\S]*invoke\('check_remote_skill_update'/);
  assert.match(refreshStatuses, /invoke\('check_remote_skill_updates'/);
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

test('remote version review labels apply actions by version action', () => {
  assert.match(appSource, /'Apply Update'/);
  assert.match(appSource, /'Apply Rollback'/);
  assert.doesNotMatch(appSource, /'Apply change'/);
});

test('remote version apply refreshes only the updated skill status', () => {
  const applyRemoteVersionChange = appSource.match(
    /async function applyRemoteVersionChange\(\)\s*\{(?<body>[\s\S]*?)\n  \}/
  )?.groups.body || '';

  assert.match(applyRemoteVersionChange, /await refreshSkillStatuses\(\{\s*skillName:\s*preview\.skillName\s*\}\);/);
  assert.doesNotMatch(applyRemoteVersionChange, /await refreshSkillStatuses\(\);/);
});

test('dashboard refresh action shows an explicit loading affordance', () => {
  assert.match(appSource, /label:\s*isChecking \? 'Refreshing' : 'Refresh'/);
  assert.match(appSource, /loading:\s*isChecking/);
  assert.match(appSource, /aria-busy=\{action\.loading \? 'true' : undefined\}/);
  assert.match(appSource, /dashboardActionButton loading/);
  assert.match(css, /\.dashboardActionButton\.loading svg\s*\{[^}]*animation:\s*syncSpin 760ms linear infinite;/s);
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

test('compact call labels stay short while usage explanations retain local scope', () => {
  const historyPageSource = appSource.match(/export function HistoryPage[\s\S]*?function HistoryRow/)?.[0] || '';
  const skillCardStart = appSource.indexOf('function SkillCard');
  const skillCardEnd = appSource.indexOf('function SkillTypeBadge', skillCardStart);
  const skillCardSource = appSource.slice(skillCardStart, skillCardEnd);

  assert.match(historyPageSource, /id:\s*'skill_usage',[\s\S]*label:\s*'Calls'/);
  assert.match(historyPageSource, /id:\s*'usage_reference',[\s\S]*label:\s*'References'/);
  assert.doesNotMatch(historyPageSource, /label:\s*'Locally observed calls'/);
  assert.match(
    historyPageSource,
    /Calls, history references, imports, updates, deploys, and other SkillBox operations will appear here\./
  );
  assert.match(historyPageSource, /filter === 'all' \? 'No history yet'/);
  assert.match(historyPageSource, /Try another history filter or sync local histories\./);
  assert.match(skillCardSource, /\{skill\.usageCount\} calls/);
  assert.doesNotMatch(skillCardSource, /locally observed calls/i);
  assert.match(appSource, /className="candidateUsage">[\s\S]*Calls \{group\.usageCount \|\| 0\}/);
  assert.match(appSource, /Calls:\s*<strong>\{workspace\.usageCount\}<\/strong>/);
  assert.match(appSource, /<strong>No calls in this range<\/strong>/);
  assert.match(appSource, /<caption className="srOnly">Skills ranked by calls<\/caption>/);
  assert.match(
    appSource,
    /Calls combine locally confirmed executions with high-confidence inferred invocations[\s\S]*not Codex or Claude[\s\S]*account analytics/
  );
  assert.match(
    appSource,
    /title="Calls combine locally confirmed executions and high-confidence inferred invocations\. They are not Codex or Claude account analytics\."/
  );
  assert.match(appSource, /History references are explicit mentions[\s\S]*never added to Calls/);
  assert.match(appSource, /Record locally observed agent skill calls from runtime hooks\./);
});

test('rankings is an accessible top-level page separate from history', () => {
  const historyPageSource = appSource.match(/export function HistoryPage[\s\S]*?function HistoryRow/)?.[0] || '';
  const rankingsPageSource = appSource.match(/export function UsageRankingsPage[\s\S]*$/)?.[0] || '';
  const rankingRangeSource = rankingsPageSource.match(
    /className="usageRankingRangeField"[\s\S]*?className="usageRankingSelect"/
  )?.[0] || '';

  assert.doesNotMatch(historyPageSource, /Rankings|usageRanking/);
  assert.match(appSource, /\{ id: 'rankings', label: 'Rankings', icon: 'chart-no-axes-column-increasing' \}/);
  assert.match(appSource, /item\.id === 'rankings'/);
  assert.match(appSource, /function openRankings\(\)/);
  assert.match(appSource, /page === 'rankings'/);
  assert.match(rankingsPageSource, /<PageTitleRow[\s\S]*title="Rankings"/);
  assert.doesNotMatch(rankingsPageSource, /subtitle=/);
  assert.match(rankingsPageSource, /onClick=\{onRefresh\}/);
  assert.match(rankingsPageSource, /panelNotice notice/);
  assert.match(rankingsPageSource, /DashboardStatusNotice/);
  assert.match(
    rankingsPageSource,
    /usageRankingControls[\s\S]*UsageCoverageDisclosure[\s\S]*Top skills by calls[\s\S]*Full ranking/
  );
  assert.doesNotMatch(
    rankingsPageSource,
    /aria-label="Rankings">\s*\{error \? <div className="notice"/
  );
  assert.match(appSource, /aria-label="Local skill usage rankings"/);
  assert.match(rankingsPageSource, /className="usageRankingSelectLabel" id="usage-ranking-range-label">[\s\S]*Time range/);
  assert.match(appSource, /className="dashboardTypeTabs usageRankingRanges"/);
  assert.match(appSource, /role="group"[\s\S]*aria-labelledby="usage-ranking-range-label"/);
  assert.match(rankingsPageSource, /aria-pressed=\{filters\.range === option\.id\}/);
  assert.doesNotMatch(rankingRangeSource, /role="tab"|aria-selected=/);
  assert.match(appSource, /<option value="">All agents<\/option>/);
  assert.match(appSource, /<option value="">All types<\/option>/);
  assert.match(appSource, /<option value="">All workspaces<\/option>/);
  assert.match(rankingsPageSource, /className="usageRankingSelectLabel">Skill type</);
  assert.match(rankingsPageSource, /className="usageRankingSelectLabel">Agent</);
  assert.match(rankingsPageSource, /className="usageRankingSelectLabel">Workspace</);
  assert.match(css, /\.usageRankingControls\s*\{[^}]*align-items:\s*center;/s);
  assert.match(css, /\.usageRankingRangeField,\s*\.usageRankingSelect\s*\{[^}]*display:\s*grid;[^}]*gap:\s*4px;/s);
  assert.match(css, /\.usageRankingRanges\s*\{[^}]*border-radius:\s*10px;/s);
  assert.match(css, /\.usageRankingRanges\s*\{[^}]*grid-template-columns:\s*repeat\(3,\s*minmax\(96px,\s*max-content\)\);/s);
  assert.match(css, /\.usageRankingRanges button\s*\{[^}]*min-width:\s*96px;/s);
  assert.match(css, /\.usageRankingRanges button\s*\{[^}]*padding:\s*0 16px;/s);
  assert.doesNotMatch(css, /\.usageRankingRanges\s*\{[^}]*min-width:\s*280px;/s);
  assert.match(css, /\.usageRankingSelect select\s*\{[^}]*border-radius:\s*10px;/s);
  assert.match(css, /\.usageRankingSelect select\s*\{[^}]*height:\s*46px;/s);
  assert.match(appSource, /Top skills by calls/);
  assert.match(appSource, /Full ranking/);
  assert.match(appSource, /Includes skills not imported into SkillBox/);
  assert.match(appSource, /Not imported/);
  assert.match(appSource, /usageRankingTopCard/);
  assert.match(appSource, /<strong>\{row\.usageCount\} calls<\/strong>/);
  assert.doesNotMatch(appSource, /<strong>\{row\.usageCount\} locally observed calls<\/strong>/);
  assert.match(appSource, /<table className="usageRankingTable">/);
  assert.match(css, /\.usageRankingTableWrap\s*\{[^}]*overflow-x:\s*auto;/s);
  assert.match(css, /\.usageRankingTable\s*\{[^}]*min-width:\s*720px;/s);
  assert.match(
    css,
    /@media \(max-width:\s*1100px\)\s*\{[\s\S]*?\.usageRankingControls\s*\{[^}]*grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\);[\s\S]*?\.usageRankingRangeField\s*\{[^}]*grid-column:\s*1 \/ -1;[\s\S]*?\.usageRankingRanges\s*\{[^}]*width:\s*100%;/s
  );
  assert.match(appSource, /<th scope="col">Rank<\/th>/);
  assert.match(appSource, /<th scope="col">Skill<\/th>/);
  assert.match(appSource, /<th scope="col">Calls<\/th>/);
  assert.match(rankingsPageSource, /useState\(false\)/);
  assert.match(rankingsPageSource, /aria-controls="usage-coverage-details"/);
  assert.match(rankingsPageSource, /aria-expanded=\{expanded\}/);
  assert.match(rankingsPageSource, /hidden=\{!expanded\}/);
  assert.match(
    rankingsPageSource,
    /\{numberOrZero\(totalCalls\)\} locally observed calls/
  );
  assert.match(rankingsPageSource, /\{numberOrZero\(totalHistoryReferences\)\} history references/);
  assert.match(rankingsPageSource, /\{expanded \? 'Hide coverage' : 'View coverage'\}/);
  assert.match(rankingsPageSource, /Coverage or ranking refresh needs attention/);
  assert.doesNotMatch(rankingsPageSource, /className="usageCoverageSummary"/);
  assert.match(rankingsPageSource, /Local data coverage/);
  assert.match(
    rankingsPageSource,
    /Earliest \$\{formatOperationTimestamp\(earliest\)\} · Latest \$\{formatOperationTimestamp\(latest\)\}/
  );
  assert.match(rankingsPageSource, /Confirmed calls/);
  assert.match(rankingsPageSource, /Inferred calls/);
  assert.match(rankingsPageSource, /History references/);
  assert.match(rankingsPageSource, /Cursor transcript files/);
  assert.match(rankingsPageSource, /Cursor state DB sessions/);
  assert.match(rankingsPageSource, /provider-reported runs remain separate/i);
  assert.match(
    rankingsPageSource,
    /Codex does not expose a\s+stable local provider total, so these auditable Calls may still undercount actual usage\./
  );
  assert.match(css, /\.usageCoverageToggle\s*\{[^}]*background:\s*transparent;/s);
  assert.match(css, /\.usageCoverageDetails\[hidden\]\s*\{[^}]*display:\s*none;/s);
  assert.match(
    css,
    /\.usageCoverageSummaryText\s*\{[^}]*flex-wrap:\s*wrap;/s
  );
  assert.match(appSource, /<th scope="col">Last observed<\/th>/);
  assert.match(rankingsPageSource, /Sync histories/);
  assert.match(rankingsPageSource, /onSyncHistories/);
  assert.match(appSource, /usageHistorySyncProviders/);
  assert.match(appSource, /await invoke\(provider\.command/);
  assert.match(appSource, /backfill_claude_code_session_usage/);
  assert.match(appSource, /backfill_cursor_session_usage/);
  assert.match(tauriSource, /async fn backfill_codex_session_usage/);
  assert.match(tauriSource, /skillbox_core::backfill_codex_session_usage/);
  assert.match(tauriSource, /async fn backfill_claude_code_session_usage/);
  assert.match(tauriSource, /async fn backfill_cursor_session_usage/);
  assert.match(tauriSource, /async fn usage_audit/);
  assert.match(tauriSource, /skillbox_core::usage_audit/);
  assert.match(appSource, /includeArchived:\s*true/);
  assert.match(appSource, /Open usage hook settings/);
  assert.match(appSource, /<th scope="col">Actions<\/th>/);
  assert.match(appSource, /Detail/);
  assert.match(appSource, /\(page === 'dashboard' \|\| page === 'rankings'\) && selectedSkill/);
  assert.match(appSource, /row\.system/);
  assert.match(
    appSource,
    /row\.system \|\| row\.sourceKind === 'unknown' \|\| row\.sourceMissing \? null/
  );
  assert.match(appSource, /button primary compactAction/);
  assert.doesNotMatch(appSource, /usageRankingActionNote/);
  assert.match(appSource, /onImportSkill/);
  assert.match(rankingsPageSource, /onImportSkill\(row\)/);
  assert.match(appSource, /preview_usage_skill_import/);
  assert.match(appComponentSource, /sourceKind:\s*row\.sourceKind/);
  assert.match(appComponentSource, /sourceId:\s*row\.sourceId/);
  assert.match(appComponentSource, /sourceRuntimeRoots:\s*row\.sourceRuntimeRoots/);
  assert.match(appComponentSource, /rankingRequest:\s*usageRankingRequest\(usageRankingFilters\)/);
  assert.match(appComponentSource, /rankingGeneratedAt:\s*usageRankings\.generatedAt/);
  assert.match(appSource, /source_id:\s*`preview:\$\{sourceKind\}:\$\{row\.skill_name\}`/);
  assert.match(appSource, /source_runtime_roots:\s*\['\/tmp\/preview-skills'\]/);
  assert.match(tauriSource, /async fn preview_usage_skill_import/);
  assert.match(tauriSource, /PreviewUsageSkillImportRequest/);
  assert.match(tauriSource, /preview_usage_skill_import_for_source/);
  assert.match(rankingsPageSource, /<PageFrame ariaLabel="Rankings">/);
  assert.match(css, /\.usageRankingActions\s*\{/);
  assert.match(appSource, /invoke\('list_skill_usage_rankings'/);
  assert.match(appSource, /function navigateToPage\(nextPage\)/);
  assert.match(appSource, /pageRef\.current = nextPage/);
  assert.match(appSource, /rankingImportRequestRef\.current \+= 1/);
  assert.match(appSource, /finally \{\s*setUsageBackfillLoading\(false\);/s);
  assert.match(
    appSource,
    /if \(rankingImportRequestRef\.current === requestId\) \{\s*setRankingImportSkillName\(''\);/s
  );
  assert.match(
    appSource,
    /if \(usageRankingRequestRef\.current === requestId\) \{\s*setUsageRankingLoading\(false\);/s
  );
  assert.match(appSource, /usageRankingRequest\(nextFilters\)/);
  assert.match(appSource, /includeUnmanaged: true/);
  assert.match(appSource, /Not imported/);
  assert.match(appSource, /Includes skills not imported into SkillBox/);
  const loadHistorySource = appComponentSource.match(/async function loadHistory\(nextFilter = getCtx\(\)\.historyFilter\)[\s\S]*?function openRankings/)?.[0] || '';
  assert.match(loadHistorySource, /invoke\('list_history'/);
  assert.doesNotMatch(loadHistorySource, /list_skill_usage_rankings|Promise\.all/);
  assert.match(tauriSource, /async fn list_skill_usage_rankings/);
  assert.match(tauriSource, /skillbox_core::list_skill_usage_rankings/);
  assert.match(css, /\.historyTypeTabs\s*\{[^}]*repeat\(4,/s);
  assert.match(
    css,
    /@media \(max-width: 1180px\) \{[\s\S]*?\.historyTypeTabs\s*\{[^}]*grid-template-columns:\s*repeat\(2,/s
  );
  assert.match(
    css,
    /@media \(max-width: 1180px\) \{[\s\S]*?\.historyTypeTabs\s*\{[^}]*height:\s*auto;[\s\S]*?min-height:\s*46px;/s
  );
  assert.doesNotMatch(css, /\.historyTypeTabs\s*\{[^}]*repeat\(3,/s);
  assert.match(css, /\.usageRankingTable\s*\{/);
  assert.match(css, /\.usageRankingTopGrid\s*\{/);
  assert.match(css, /\.usageRankingTopCard\.leader\s*\{/);
  assert.match(css, /\.usageRankingRanges\s*\{/);
  assert.match(css, /\.dashboardTypeTabs button\.active/);
  assert.match(css, /font-variant-numeric:\s*tabular-nums/);
});

test('skill card favorite action stays above the full-card hit area', () => {
  const skillCardStart = appSource.indexOf('function SkillCard');
  const skillCardEnd = appSource.indexOf('function AgentIconStack', skillCardStart);
  const skillCardSource = appSource.slice(skillCardStart, skillCardEnd);
  const favoriteRule = css.match(/\.skillFavoriteButton\s*\{(?<body>[^}]*)\}/s)?.groups.body || '';

  assert.match(skillCardSource, /className="skillCardMetaDetails"[\s\S]*<Badge tone=\{skill\.statusTone\}>\{skill\.statusLabel\}<\/Badge>/);
  assert.match(skillCardSource, /className=\{skill\.isFavorite \? 'skillFavoriteButton active' : 'skillFavoriteButton'\}/);
  assert.match(skillCardSource, /onPointerDown=\{\(event\) => event\.stopPropagation\(\)\}/);
  assert.match(skillCardSource, /onClick=\{\(event\) => \{[\s\S]*event\.stopPropagation\(\);[\s\S]*void onToggleFavorite\(skill\.name\);/);
  assert.doesNotMatch(skillCardSource, /skillCardHeaderActions/);
  assert.match(favoriteRule, /position:\s*absolute;/);
  assert.match(favoriteRule, /top:\s*20px;/);
  assert.match(favoriteRule, /right:\s*18px;/);
  assert.match(favoriteRule, /z-index:\s*3;/);
  assert.match(favoriteRule, /pointer-events:\s*auto;/);
  assert.match(favoriteRule, /width:\s*32px;/);
  assert.match(favoriteRule, /height:\s*32px;/);
});

test('installed workspace icons use immediate custom tooltips instead of native title delay', () => {
  assert.match(appSource, /const visibleAgents = agents\.slice\(0, 3\);/);
  assert.match(appSource, /\+\{overflowCount\}/);
  assert.match(appSource, /data-tooltip=\{agent\.label\}/);
  assert.match(appSource, /aria-label=\{agent\.label\}/);
  assert.doesNotMatch(appSource, /className="skillAgentIcons" aria-label=\{label\} title=\{label\}/);
  assert.match(css, /\.skillAgentIcon\[data-tooltip\]::after\s*\{/);
  assert.match(css, /\.skillAgentIcon\[data-tooltip\]:hover::after,\s*\.skillAgentIcon\[data-tooltip\]:focus-visible::after\s*\{/);
  assert.doesNotMatch(css, /\.skillAgentIcon\[data-tooltip\]::after\s*\{[^}]*transition-delay:\s*[1-9]/s);
});

test('skill detail can request a confirmed skill type change', () => {
  assert.match(tauriSource, /async fn change_skill_kind\([\s\S]*skill_name:\s*String,[\s\S]*skill_type:\s*skillbox_core::SkillKind,[\s\S]*\)/);
  assert.match(tauriSource, /skillbox_core::change_skill_kind\([\s\S]*&skill_name,[\s\S]*skill_type,/);
  assert.match(tauriSource, /change_skill_kind,/);
  assert.match(appSource, /const \[skillTypeChangeDialog,\s*setSkillTypeChangeDialog\]\s*=\s*useState/);
  assert.match(appSource, /async function confirmSkillTypeChange\(\)/);
  assert.match(appSource, /invoke\('change_skill_kind',\s*\{\s*skillName:\s*skillTypeChangeDialog\.skillName,\s*skillType:\s*skillTypeChangeDialog\.targetType\s*\}\)/);
  assert.match(appSource, /export function ConfirmDialog\(/);
  assert.match(appSource, /function SkillTypeChangeDialog/);
  assert.match(appSource, /<ConfirmDialog[\s\S]*className="skillTypeChangeDialog"[\s\S]*confirmLabel="Confirm change"/);
  assert.match(appSource, /Confirm type change/);
  assert.doesNotMatch(appSource, /className="remoteImportDialog skillTypeChangeDialog"/);
  assert.match(appSource, /onRequestTypeChange=\{openSkillTypeChangeDialog\}/);
  assert.match(appSource, /<section className="skillDetailControlSection skillDetailTypeControl" aria-label="Skill type">/);
  assert.match(appSource, /className="skillDetailTypeSegment"/);
  assert.doesNotMatch(appSource, /className="candidateTypeToggle skillDetailTypeToggle"/);
  assert.match(appSource, /onRequestTypeChange\(skill,\s*'remote'\)/);
  assert.match(appSource, /onRequestTypeChange\(skill,\s*'user'\)/);
  assert.match(css, /\.skillDetailTypeSegment\s*\{[^}]*background:\s*var\(--skillbox-slate-bg\);/s);
  assert.match(css, /\.skillDetailTypeSegment button\.active\s*\{[^}]*background:\s*var\(--skillbox-blue\);/s);
  assert.match(css, /\.skillDetailTypeSegment button\.active\s*\{[^}]*color:\s*var\(--skillbox-surface\);/s);
  assert.match(css, /\.skillDetailTypeSegment button:disabled\s*\{[^}]*opacity:\s*1;/s);
  assert.match(css, /\.confirmDialog\s*\{[^}]*width:\s*min\(520px,\s*calc\(100vw - 64px\)\);/s);
  assert.match(css, /\.confirmDialogHeader\s*\{[^}]*border-bottom:\s*1px solid var\(--skillbox-slate-bg\);/s);
  assert.match(css, /\.confirmDialogFooter\s*\{[^}]*border-top:\s*1px solid var\(--skillbox-slate-bg\);/s);
  assert.match(css, /\.skillTypeChangeSummary\s*\{[^}]*background:\s*var\(--skillbox-surface-blue-subtle\);/s);
  assert.match(appSource, /<Badge tone="slate">\{currentLabel\} skill<\/Badge>[\s\S]*<Badge tone="slate">\{targetLabel\} skill<\/Badge>/);
});

test('skill detail favorite action lives in the header actions', () => {
  assert.match(appSource, /<div className="skillDetailHeaderActions">[\s\S]*className=\{skill\.isFavorite \? 'detailFavoriteButton active' : 'detailFavoriteButton'\}/);
  assert.match(appSource, /<div className="skillDetailHeaderActions">[\s\S]*onClick=\{\(\) => onToggleFavorite\(skill\.name\)\}/);
  assert.doesNotMatch(appSource, /<footer className="skillDetailActions">[\s\S]*detailFavoriteButton/);
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
  assert.match(css, /\.remoteVersionRow \.remoteVersionCurrentBadge\s*\{[^}]*color:\s*var\(--skillbox-green-text\);/s);
  assert.match(css, /\.remoteVersionRow \.remoteVersionCurrentBadge\s*\{[^}]*pointer-events:\s*none;/s);
});

test('local skill detail renders version history from the user skills git repo', () => {
  assert.match(tauriSource, /fn list_user_skill_versions\(skill_name:\s*String\)/);
  assert.match(appSource, /invoke\('list_user_skill_versions',\s*\{\s*skillName\s*\}\)/);
  assert.match(appSource, /function UserSkillVersionHistoryPanel/);
  assert.match(appSource, /skill\.type === 'user' \? \(\s*<UserSkillVersionHistoryPanel/s);
  assert.match(appSource, /aria-label="User skill version history"/);
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

test('remote source candidate view opens through the desktop bridge with a browser fallback', () => {
  assert.match(appSource, /async function viewRemoteSourceCandidate\(candidate\)/);
  assert.match(appSource, /invoke\('open_external_url'/);
  assert.match(appSource, /window\.open\(sourceUrl,\s*'_blank',\s*'noopener,noreferrer'\)/);
});

test('workspace deployment removal is explicit about removing only managed symlinks', () => {
  assert.match(appSource, /Unchecked skills will be removed from these workspaces/);
  assert.match(appSource, /Confirm removal from \{changes\.undeploy\.length\} workspace/);
  assert.match(appSource, /existing directories or foreign symlinks are refused/);
});

test('workspace setup previews project roots before creating or registering one target', () => {
  assert.match(appSource, /invoke\('preview_workspace_setup'/);
  assert.match(appSource, /invoke\('apply_workspace_setup'/);
  assert.match(appSource, /selected_path:\s*workspacePath/);
  assert.match(appSource, /selected_root:\s*selectedRoot\.path/);
  assert.match(appSource, /create_missing:\s*!selectedRoot\.exists/);
  assert.match(appSource, /preview_id:\s*preview\.previewId/);
  assert.match(appSource, />\s*Project\s*<\/button>/);
  assert.match(appSource, />\s*Global\s*<\/button>/);
  assert.match(appSource, /Project or skills folder/);
  assert.match(appSource, /No skills folder found/);
  assert.match(appSource, /Only <code>\{selectedRoot\.path\}<\/code> will be created/);
  assert.match(appSource, /Existing project files will not be changed/);
  assert.match(appSource, /'Create & add'/);
  assert.match(appSource, /onSelectRoot\(root\.path\)/);
  assert.doesNotMatch(appSource, /invoke\('add_workspace'[\s\S]*submitWorkspaceDialog/);
  assert.match(css, /\.workspaceSetupRoots\s*\{[^}]*display:\s*grid;/s);
});
