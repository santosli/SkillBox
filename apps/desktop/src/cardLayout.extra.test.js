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

