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
  './appActionsB.js',
  './appActionsC.js',
  './appActionsD.js'
];
const appSource = (
  await Promise.all(
    appSourcePaths.map((path) => readFile(new URL(path, import.meta.url), 'utf8'))
  )
).join('\n');
const appComponentSource = (
  await Promise.all(
    ['./App.jsx', './appActionsA.js', './appActionsB.js', './appActionsC.js', './appActionsD.js'].map((path) =>
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
  assert.match(groupSource, /id=\{typeLabelId\}>Skill type/);
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
