# SkillBox 数据模型

## Managed Store 布局

默认根目录是 `~/.skillbox`，也可以通过 `SKILLBOX_HOME` 指向其它目录。
managed store 是跨 agent 的真相源，不绑定 Codex、Claude、Cursor、Copilot 或任何单一 runtime。
历史版本使用过 `~/SkillBox`。当 `SKILLBOX_HOME` 未设置、`~/.skillbox` 只是空的启动壳、
且 `~/SkillBox` 已有 managed data 时，Rust core 会先备份空壳目录，再创建
`~/.skillbox -> ~/SkillBox` 兼容链接。这样 UI 和 CLI 继续使用隐藏路径，
同时保留旧 runtime symlink 指向 `~/SkillBox` 时的可用性。

```text
~/.skillbox/
  user-skills/
    <skill-name>/
      SKILL.md
  remote-skills/
    <skill-name>/
      source.json
      current -> versions/<version>
      versions/
        <version>/
          SKILL.md
  backups/
    imports/
      <skill-name>-<contentHash12>/
    deletions/
      <skill-name>-<timestamp>/
    deletion-conflicts/
      <skill-name>-<timestamp>
  adapters/
    <agent-id>/
  skillbox.sqlite
  skillbox.sqlite.pre-migration-v<version>-<timestamp>.bak
```

规则：

- `user-skills/<skill-name>` 保存用户创建或本地导入的 skill。
- `remote-skills/<skill-name>/versions/<version>` 保存远程或手动远程导入的不可变快照。
- `remote-skills/<skill-name>/current` 指向当前生效版本。
- `backups/imports` 保存从 runtime 目录迁移到 SkillBox 前的原始内容。
- `backups/deletions` 保存从 managed store 删除的 user skill 目录或完整 remote skill root，供误删恢复；删除 workflow 不自动清理这些备份。
- `backups/deletion-conflicts` 保存删除期间因并发替换而无法放回 workspace 的未知 target；SkillBox 不自动删除这些内容。若跨卷迁移失败，Doctor 会报告仍留在 workspace 的 `.delete-check-*.tmp` 路径，要求人工检查。
- `adapters/<agent-id>` 预留给 agent-specific cache、manifest 或转换产物；当前 Rust schema 尚未实现。
- 已存在的数据库进入新 schema migration 前，通过 SQLite 一致性快照生成一次 `pre-migration` backup；同一 schema version 不重复备份。初始化会先获取 per-database process-safe migration lock，再判断是否需要 backup/migration，确保并发 desktop/CLI caller 只生成一份 backup 并执行一次有序迁移。
- 一个有效 skill 目录必须包含 `SKILL.md`。
- workspace 表记录 skills 所在工程目录或 runtime skills root，用于后续部署目标选择；workspace path 指向
  `.../.agents/skills`、`.../.codex/skills`、`.../.claude/skills`、
  `.../.cursor/skills` 或手动登记的 exact `SKILL.md` root，而不是单个 skill 目录。
- 整体删除 skill 时只清理 `skills`、该 skill 的 `deployments`、`skill_user_metadata` 和 remote update cache 中的当前状态；保留 `workspaces`、`operations`、usage history 以及已结束的 import history。存在 active import record 时拒绝删除。

当前实现仍以 `SKILL.md` 目录作为可读写单位。Claude、OpenClaw、Cursor、Claude Code、Copilot 等 agent 可能使用不同的原生文件格式；
支持这些格式时，应由 adapter 把原生格式映射到 SkillBox 的规范化记录，而不是让 UI 或 workflow 分别维护 schema。

## User-Skills Git State And Recovery

`~/.skillbox/user-skills` 同时是 user skill managed root 和一个共享 Git
repository。v0.7 入站同步不新增 SQLite schema；Git commit/ref 是 repository
内容 identity，SQLite `skills` rows 是可重建索引。

入站 status 分成两个正交维度：

- worktree：`clean` 或 `dirty`；
- relation：`unknown`、`synced`、`ahead`、`behind`、`diverged`、
  `remote_only` 或 `no_remote_branch`。

Status/preview 同时返回 local SHA、remote SHA、merge-base SHA、
ahead/behind counts、`origin/main` branch、sanitized remote URL 和 fetch
timestamp/error。`dirty + behind` 等组合是合法状态：可以 review，但不能 apply。

Preview 是 repository-wide snapshot，不是逐 skill patch。`preview_id` 至少绑定
local HEAD、remote SHA、merge base、remote URL、branch、worktree state、validated
remote tree、file/skill changes 和 affected deployments。Apply 会重新 fetch 并重算；
任一 identity/state 变化都必须拒绝旧 preview。

Apply 前，已有 local HEAD 会保存到：

```text
refs/skillbox/backups/inbound/<operation-id>
```

随后只允许 `behind` 的 fast-forward，或在本地没有 user content 时执行
`remote_only` bootstrap。Git 成功后，Rust core 在一个 SQLite transaction 中删除并
重建 `type=user` 的 skill index，使其对应新的完整 repository snapshot。remote skill
rows、workspace registry、usage history 和 operation history 不属于该 rebuild。

这是 Git + SQLite 两种持久化边界的补偿式 saga，不是一个跨系统 transaction：

- remote tree validation 在 working-tree write 前完成；
- incoming paths 与 ignored/untracked local content 的 exact/ancestor/descendant
  collision 在 write 前阻止；
- validated Git blobs 直接物化到受审路径，index 更新后以 compare-and-swap 推进
  `main`；仓库 hook、filter、textconv、external diff 和 merge driver 不参与；
- apply 持有 `.git/index.lock`，并在替换/删除 tracked 文件前把旧 worktree 内容以
  fd-relative no-replace rename 保存到 `.git/skillbox/inbound-worktree-backups/`；
  recovery snapshot 的每级 parent 通过 no-follow directory handle 打开，路径在验证
  后被换成 symlink 也不能把 backup/restore 重定向到 repo 外；snapshot 与 backup ref
  一起保留，不进入 runtime 或 skill index；
- user-skills Git、managed skill mutation 与 deploy/undeploy 共享 mutation lock；
- 文件写入、ref 更新或 reindex 失败时，core 只在 HEAD 与操作写集仍匹配预期状态时
  补偿；发现外部 mutation 时拒绝覆盖恢复；
- remote-only 补偿独立尝试删除本次写入、清空 index 和恢复原有 generated
  `.gitignore` setup state；恢复 `.gitignore` 使用 no-replace，遇到并发外部内容时保留
  外部文件并记录 partial recovery，使失败步骤不会跳过其它可执行恢复；
- generated `.gitignore` 只由持有 mutation lock 的显式 Git 配置/同步流程写入；
  通用 managed-layout/read 初始化不修改 Git worktree；
- internal backup ref 保留，便于人工 recovery；
- compensation 失败会作为 actionable error/operation result 暴露，不能静默忽略；
- failure operation payload 保存 old/new SHA、backup ref、mutation phase 和
  compensation outcome；不保存 credentials、diff content 或 skill body。

已部署 skill 的 update 可在 preview 明确列出 target 后应用。已部署 skill 的 delete
或 rename 在 v0.7 阻止 apply，要求先 undeploy；这样 runtime symlink 不会因 repository
级变更成为 dangling target。无 local history 且只有生成 `.gitignore` 的 repository
可以 safe bootstrap；已有 user content 时必须阻止。

## Remote Source Metadata

远程 skill 的来源元数据保存在 `remote-skills/<skill-name>/source.json`。

GitHub remote 使用这些字段：

```json
{
  "type": "github",
  "owner": "openai",
  "repo": "skills",
  "path": "skills/example",
  "root": false,
  "ref": "main",
  "refKind": "branch",
  "tracking": true,
  "repoUrl": "https://github.com/openai/skills.git",
  "url": "https://github.com/openai/skills/tree/main/skills/example",
  "currentVersion": "manual-<contentHash12>",
  "installedSha": null,
  "latestSha": "full-commit-sha",
  "installedAt": "2026-05-23T00:00:00.000Z"
}
```

GitHub source 的版本语义：

- `refKind: "branch"` 且 `tracking: true` 表示跟踪分支，update check 会查询远端最新 SHA。
- 目录 source 使用非空 `path` 和 `root: false`。根目录包含 `SKILL.md` 的 standalone repository 使用空 `path` 和 `root: true`，其 managed version 是移除 `.git` metadata 后的完整 worktree。
- 旧 `source.json` 没有 `root` 字段时按 `false` 读取，因此现有目录 source 不需要 migration。
- `refKind: "tag"` 或 `refKind: "commit"` 表示 pinned source，update check 返回 `pinned`，不会自动判断有可用更新。
- `currentVersion` 是当前 `current` symlink 指向的 managed version 目录名，可以是 `manual-*` 版本，也可以是 GitHub commit SHA。
- `installedSha` 只在当前版本来自 GitHub commit 时设置；手动绑定远端但尚未替换内容时保留为 `null`。
- `latestSha` 是最近一次远端查询到的 GitHub SHA，可作为展示缓存，不代表已经安装。

Manual remote 使用这些字段：

```json
{
  "type": "manual",
  "installedSha": "manual-<contentHash12>",
  "installedAt": "2026-05-23T00:00:00.000Z"
}
```

当前差异：

- Node remote import/install 会写 `source.json`。
- Rust remote import 当前会写 `versions/<manual-version>` 和 `current` symlink，但尚未写 `source.json`。
- Rust 迁移 GitHub install/update/rollback 时，必须补齐 `source.json` 写入和兼容读取。

## SQLite

数据库文件是 `~/.skillbox/skillbox.sqlite`。

Rust 当前表：

```text
schema_migrations
  version INTEGER PRIMARY KEY
  name TEXT NOT NULL
  applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP

skills
  name TEXT PRIMARY KEY
  type TEXT NOT NULL
  description TEXT NOT NULL DEFAULT ''
  version TEXT NOT NULL DEFAULT ''
  managed_path TEXT NOT NULL
  status TEXT NOT NULL DEFAULT 'ok'
  content_hash TEXT NOT NULL DEFAULT ''
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP

deployments
  skill_name TEXT NOT NULL
  target_root TEXT NOT NULL
  target_path TEXT NOT NULL
  mode TEXT NOT NULL
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
  PRIMARY KEY (skill_name, target_root)

preferences
  key TEXT PRIMARY KEY
  value TEXT NOT NULL
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP

workspaces
  canonical_path TEXT PRIMARY KEY
  path TEXT NOT NULL
  kind TEXT NOT NULL
  source TEXT NOT NULL
  agent_id TEXT
  profile_id TEXT NOT NULL DEFAULT 'custom-skill-md'
  root_key TEXT NOT NULL DEFAULT 'exact'
  format TEXT NOT NULL DEFAULT 'skill_md'
  display_name TEXT NOT NULL
  skill_count INTEGER NOT NULL DEFAULT 0
  imported_skill_count INTEGER NOT NULL DEFAULT 0
  last_scan_error_count INTEGER NOT NULL DEFAULT 0
  last_scan_error TEXT
  last_scanned_at TEXT
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP

operations
  id TEXT PRIMARY KEY
  type TEXT NOT NULL
  status TEXT NOT NULL
  actor TEXT NOT NULL
  entity_type TEXT NOT NULL
  entity_name TEXT NOT NULL
  started_at TEXT NOT NULL
  finished_at TEXT
  summary TEXT NOT NULL
  error TEXT
  payload_json TEXT NOT NULL

import_records
  id TEXT PRIMARY KEY
  skill_name TEXT NOT NULL
  type TEXT NOT NULL
  source_path TEXT NOT NULL
  source_root TEXT
  managed_path TEXT NOT NULL
  content_hash TEXT NOT NULL
  backup_path TEXT NOT NULL
  deployed_path TEXT NOT NULL
  status TEXT NOT NULL
  legacy INTEGER NOT NULL DEFAULT 0
  imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
  reverted_at TEXT

skill_usage_events
  id TEXT PRIMARY KEY
  event_id TEXT
  skill_name TEXT NOT NULL
  agent_id TEXT NOT NULL
  runtime_root TEXT NOT NULL
  used_at TEXT NOT NULL
  recorded_at TEXT NOT NULL
  prompt_excerpt TEXT
  metadata_json TEXT NOT NULL DEFAULT '{}'
  evidence_class TEXT NOT NULL DEFAULT 'reference'
  evidence_sources_json TEXT NOT NULL DEFAULT '[]'
  INDEX (used_at, skill_name)
  INDEX (agent_id, used_at, skill_name)
  INDEX (runtime_root, used_at, skill_name)
  INDEX (agent_id, runtime_root, used_at, skill_name)
  INDEX (evidence_class, used_at, skill_name)
  INDEX (evidence_class, agent_id, used_at, skill_name)
  INDEX (evidence_class, runtime_root, used_at, skill_name)

skill_usage_stats
  skill_name TEXT NOT NULL
  agent_id TEXT NOT NULL
  runtime_root TEXT NOT NULL
  usage_count INTEGER NOT NULL DEFAULT 0
  last_used_at TEXT NOT NULL
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
  PRIMARY KEY (skill_name, agent_id, runtime_root)

skill_user_metadata
  skill_name TEXT PRIMARY KEY
  favorite INTEGER NOT NULL DEFAULT 0
  tags_json TEXT NOT NULL DEFAULT '[]'
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
```

`schema_migrations` 是 Rust schema 的唯一版本历史。migration 按 version 顺序在独立 transaction 中执行；已有数据库在首次执行待处理 migration 前生成一致性 backup，全部 migration 完成后运行 SQLite integrity check。新建空数据库不生成 backup。backup decision 和 migration application 由同一个进程安全文件锁串行化，锁随文件句柄释放，即使进程异常退出也不会留下永久锁状态。schema v6 增加 workspace runtime profile identity，并按 `canonical_path` 的 component suffix backfill：`.agents/skills` -> `agents`、`.codex/skills` -> `codex`、`.claude/skills` -> `claude-code`、`.cursor/skills` -> `cursor`；其它已登记 root -> `custom-skill-md`。因此，显示路径看似 built-in root、但 symlink 实际解析到其它位置的 legacy workspace 会按 canonical identity 迁移为 `custom-skill-md/exact`。全部现有 row 使用 `format=skill_md`，不要求重新 scan。schema v7 为 usage event 增加 evidence class 与有界 provenance：已有事件按可信 source 保守回填为 `confirmed`、`inferred` 或 `reference`，随后在同一 migration transaction 中幂等重建 `skill_usage_stats`，不删除 raw event，也不要求重新扫描本地 history。legacy Claude session rows 因缺少逐条 native Skill attribution 而先保守迁为 `inferred`；用户之后显式执行 `Sync histories` 时，才会从仍可用的本地 session 文件恢复或升级 native Skill tool/command evidence 为 `confirmed`。

`skill_user_metadata` 保存用户显式设置的 favorite 和 tags。桌面首次读取该表时会把旧 `localStorage` 中仍存在的 metadata 通过 `INSERT OR IGNORE` 迁入，因此 SQLite 中已有值不会被旧浏览器状态覆盖；迁移成功后删除旧 key。

当前 `preferences` key-value 表除用户设置和 remote skill update cache 外，还保存
`app_update_check_cache`、`codex_usage_backfill_scanned_files`、
`codex_usage_backfill_scanned_turns`、`claude_code_usage_backfill_scanned_files`、
`cursor_usage_backfill_scanned_sessions`、
`cursor_usage_backfill_scanned_transcript_files` 和
`usage_backfill_audit_<source>`。`app_update_check_cache` 只记录最近一次成功的 updater metadata check 展示快照
（current/available version、release date/body、checked time 和 message），用于 24 小时
节流与跨启动恢复提醒。它不保存下载 URL、签名或安装授权；损坏、时间异常或 current
version 不匹配时必须忽略并重新通过 Tauri updater plugin 检查；下载和安装阶段仍由
plugin 校验 updater asset 签名。usage coverage keys 保存最近一次 Codex、Claude Code 和
Cursor history sync 扫描的 rollout/project JSONL 文件数、Codex turn 数、Cursor composer
session 数或 Cursor agent transcript 文件数；backfill audit 仅保存 aggregate
discovered/recorded/deduplicated/upgraded/skipped/errors。它们不随 Rankings 的时间、
skill type、Agent 或 Workspace 过滤器变化，也不包含聊天正文。

`workspaces.profile_id/root_key/format` 是 deployment target identity。registry v1
包含 `agents`、`codex`、`claude-code`、`cursor` 和
`custom-skill-md`；built-in roots 使用 `root_key=skills`，手动 exact root 使用
`root_key=exact`。`agent_id` 暂时保留供旧数据库/调用方兼容，不能替代 profile，也不与
usage event 的 `agent_id` 合并。`workspaces.display_name` 由 path 推导：home-level
global roots 使用 profile 名，项目局部 roots 使用项目目录名（例如
`demo-vault`）。`global` / `user` 不拼进名称，由 `kind` 字段表达。
`imported_skill_count` 使用 import candidate 的同一套 imported 判定：workspace
skill 必须是指向 SkillBox managed root 的 symlink；仅内容 hash 匹配 managed store
不再表示该 runtime 位置仍被 SkillBox 管理。

Runtime profile registry 不存为用户可编辑 JSON。profile 声明 roots、precedence、
`skill_md` format、frontmatter policy 和当前 `symlink` deployment mode。
compatibility preview 是派生的只读结果，不另建持久表；其 `preview_id` 绑定 skill
完整目录 snapshot、target canonical path/state、profile registry version、
`profile_id/root_key/format` 和 deployment mode。unknown optional frontmatter
保留在原始 `SKILL.md` 中并返回 warning，compatibility engine 不 rewrite source。

`operations` 记录会改变用户 skill 内容、managed store、runtime、Git state、workspace registry 或 hook 配置的主要动作。Rust core 统一写入，UI 只能读取展示或通过结构化命令触发新记录；记录从 UI 视角 append-only，MVP 不做自动清理。`payload_json` 保存操作细节，例如 from/to version、changed paths、backup path/ref、affected deployments、old/new commit SHA、aggregate changed counts 或失败恢复状态，但不保存 Git credentials、review diff/skill body 或 hook 配置正文。低风险 UI metadata、remote fetch/ref refresh、自动 cache/index refresh 和纯读取操作不写 operation history。

`import_records` 记录本地 import 且 deploy back 到 source 成功后的可恢复状态。每个 imported skill 一条记录，`source_path` 是被替换成 SkillBox symlink 的 runtime 原路径，`backup_path` 是 import 前移动到 `backups/imports` 的原目录。`status=active` 的记录可以通过 `revert_import` 恢复；`status=reverted` 表示 backup 已恢复回 source path。`legacy=1` 表示记录由旧 deployments/backups 证据链保守 reconcile 得到。

`skill_usage_events` 保存本机 usage evidence，不假定每条 row 都是一次已确认执行，也不记录 SkillBox 打开详情、部署、更新等管理行为。schema v7 的 `evidence_class` 是该 canonical event 当前最强证据：`confirmed` 证明本机执行/加载，`inferred` 证明结构化逐回合调用意图，`reference` 只证明提及或上下文附加。用户可见 `Calls = confirmed + inferred`；reference 单独显示为 History references。`event_id` 是可选幂等键，在同一 canonical `agent_id + runtime_root` 下重复上报不会创建第二条 event（去重同时兼容遗留 `agents`/`claude`）。

`evidence_sources_json` 是最多八项的有界 provenance list，每项保存 source 与该来源提供的 evidence class。相同 canonical event 收到更强信号时只执行 `reference -> inferred -> confirmed` 单向升级，不增加 event 数；较弱信号不能降级，所有已观测 source 继续保留。因而 event 的当前 evidence-class totals 是互斥 partition，而 provenance source counts 可以重叠，其总和不得被要求等于 Calls 或总事件数。hook/backfill 还写入 `metadata.skill_source_kind=regular|system`，让同一 runtime root 下的同名普通/System skill 保持独立身份。只有 core 内部 trusted 入口可以写保留 source，或在 deployment attribution 改变时按 canonical agent aliases、`skill_name + event_id` 复用首次 runtime root；公开 `usage-record` 默认写 `reference`，metadata 不能获得这些权限。

来源证据按 [ADR 0005](decisions/0005-usage-evidence-classification.md) 分类：

- Stop hook `agent_hook` 是 `confirmed`。
- Codex `rollout-*.jsonl` 中完整 user-turn `<skill><name>/<path>` block 或 `[$skill](.../SKILL.md)` link，在绝对 `SKILL.md` 路径校验后是 `inferred`。它证明 per-turn invocation intent，但不是 provider-native execution result。
- Claude Code project JSONL 中原生 Skill tool use / Skill command attribution，在真实 `SKILL.md` 校验后是 `confirmed`。
- Cursor history SQLite 中 non-subagent human bubble 的 `addedWithoutMention=false context.cursorRules[].filename` 是 `reference`；它只证明 skill 被附加为上下文。
- Cursor agent transcript 中 assistant `tool_use` 的 `Read` 是 `inferred`，按稳定 transcript identity + user turn + skill/path identity 去重；同一 user turn 对同一 skill 的重复 Read 只算一次。现存文件必须通过 traversal、symlink、allowed-root、regular-file、大小和 `SKILL.md` frontmatter 检查。文件后来移动或删除时，只允许位于安全本机边界内、basename 精确为 `SKILL.md`、parent skill name 合法且最近现存 ancestor 未逃逸的 lexical historical path；该 evidence 不能用于任何文件系统或 deploy 决策。`ReadFile` candidates 单独进入 aggregate diagnostics，qualification 前不写 Calls。
- catalog、普通 user/assistant prose、裸 `SKILL.md` mention、`exec_command`、custom/dynamic tool payload、tool/shell output 均不进入 Calls。

所有 history provider 都不保存聊天或 rule 正文，并用稳定 provider/session/turn/path identity 幂等去重。Codex session metadata 的 `cwd`、Claude project path 和 Cursor workspace 用于恢复 workspace identity。`prompt_excerpt` 仅供可信实时 hook 或明确允许的 Codex user prompt 摘要使用，必须剥离 skill carrier、压缩空白并限制长度；Claude Code/Cursor 回填不写 prompt excerpt。`metadata_json` 只接受小型 JSON object，不保存 prompt、聊天正文、文件内容或 diff。Cursor private SQLite 使用 `query_only` 和短 busy timeout，不兼容 schema fail closed；Cursor transcript reader 还有目录深度、文件/行大小、候选数和 allowed-root 上限。

schema v4 增加 ranking 查询索引；schema v5 规范 legacy usage agent ids 并删除相同 event identity 的重复 row；schema v7 在 migration transaction 中保守回填 `evidence_class/evidence_sources_json`、增加 evidence indexes，并从 `confirmed + inferred` events 幂等重建 `skill_usage_stats`。迁移沿用升级前 backup 和 integrity check，不扫描 agent history，也不要求 rescan；用户之后显式运行 `Sync histories` 时，可按稳定 identity 恢复新 evidence 或升级旧 event。

`skill_usage_stats` 按 `skill_name + agent_id + runtime_root` 保存 all-time Calls 聚合，只包含 `confirmed + inferred`，继续服务详情页、skill card 和 workspace Calls。History references 直接从 reference events 聚合，不进入 stats。7 天、30 天和带 skill type/Agent/Workspace 过滤的 Rankings 必须从 `skill_usage_events` 聚合；同一过滤快照返回 `total_calls`、confirmed/inferred/reference totals 和各自时间覆盖。skill type 只包含 User、Remote、System，且必须在 coverage 累计前过滤。排名不返回 `prompt_excerpt` 或完整 `metadata_json`。

coverage 的 evidence-class totals 按 event 当前最强 class 互斥：`confirmed + inferred = Calls`，reference 单列。`source_counts` 按 `evidence_sources_json` 统计 provenance，因此同一升级事件可同时出现在 Codex inferred 与 hook confirmed source 下；source counts 不是互斥 Calls 分解。最近一次 provider scan 文件/session/turn 数是独立操作覆盖，不随 ranking filters 改变。Ranking row 继续携带稳定 `source_id`、`source_kind` 和排序后的 `source_runtime_roots` 供 source-aware Import；同名普通/System/unknown source 和 deleted source 的现有隔离规则不变。

Codex 当前本地 stores 不提供稳定、专用的 provider-native skill-run total。因此 Codex Calls 是 hook-confirmed 加结构化 per-turn inferred invocation 的本机下界，已知可能 undercount；SkillBox 不从 catalog、prose、shell/tool payload 或 output 补数。未来若接入 Codex reported runs，必须使用独立于 `skill_usage_events` / `skill_usage_stats` 的存储和读取模型，并保存 provider、subject kind、time window、scope 与 provenance；不得写入本地 ranking、total 或 delta，也不能与 Calls 换算、补差或去重。

History 是只读聚合视图，不新增表。Rust core 从 `operations` 和 `skill_usage_events` 读取最近记录，按事件时间合并为桌面时间线；Calls 和 History references 使用独立 count/filter/kind，reference 不得显示为 Call。History 只展示摘要字段，不向 React 暴露 operation payload、usage metadata 或 evidence provenance body。

`usage-audit` 同样不新增表。它只读取 event class/provenance 与 provider scan/backfill preferences，返回 confirmed/inferred/reference totals、时间覆盖、source counts、扫描文件/session/turn 数，以及最近 backfill 的 discovered/recorded/deduplicated/upgraded/skipped/errors。响应不得包含 prompt、chat body、tool payload/output、credentials 或完整 metadata；Codex provider-native total 不可用时显式返回 limitation，而不是推导补数。

usage hook 注入状态不写入 SQLite。SkillBox 设置页读取并更新各 agent 自己的 hook 配置文件。注入命令由 SkillBox 生成，固定指向 `~/.skillbox/bin/skillbox-usage-hook <agent>`。这个 wrapper 由 SkillBox 写入，并调用同目录的 `skillbox-usage-hook-runner`；安装或重新注入时会刷新 runner，避免 hook 配置依赖开发态 `target/debug` 路径或 legacy Node CLI：

- Codex App 和 Codex CLI：`~/.codex/hooks.json`，注入 Codex usage hook command 到 `hooks.Stop`。
- Claude Code CLI：`~/.claude/settings.json`，注入 Claude Code usage hook command 到 `hooks.Stop`。

安装 hook 前会备份已有配置文件；后续状态展示直接读取配置文件中是否已经包含对应 command。

跨 agent 目标 schema 需要补充的概念：

```text
agents
  id TEXT PRIMARY KEY
  display_name TEXT NOT NULL
  adapter TEXT NOT NULL
  status TEXT NOT NULL

runtime_targets
  id TEXT PRIMARY KEY
  agent_id TEXT NOT NULL
  scope TEXT NOT NULL
  path TEXT NOT NULL
  format TEXT NOT NULL

deployments
  skill_name TEXT NOT NULL
  target_root TEXT NOT NULL
  target_path TEXT NOT NULL
  mode TEXT NOT NULL
  agent_id TEXT
  target_id TEXT
  updated_at TEXT NOT NULL
```

这不是当前已实现 schema。新增 agent 支持时应先设计 migration，再让 Rust core 统一读写。

当前已实现的 `workspaces` registry 是 `runtime_targets` 的前置模型：

- `kind=global` 表示 home-level skills root，例如 `~/.agents/skills`、
  `~/.codex/skills`、`~/.claude/skills`、`~/.cursor/skills`。
- `kind=user` 表示用户项目局部 skills root，例如 `<project>/.agents/skills`。
- `source=auto` 表示由 scan 自动发现；`source=manual` 表示用户显式添加。
- `source=manual` 可以来自现有 skills-root 注册，也可以来自 desktop workspace setup 的 preview-confirmed project-local root 初始化。UI 的 `Project` 仍写入 `kind=user`，因此不改变现有 enum 或 schema。
- 初始化只允许 registry v1 的 `<project>/.agents/skills`、
  `<project>/.codex/skills`、`<project>/.claude/skills`、
  `<project>/.cursor/skills`；preview 不写磁盘，apply 每次最多创建一个选中的
  root。删除 manual workspace 只删除 registry 记录，不删除文件。
- `canonical_path` 用于去重，`path` 保留展示路径。

Node MVP 旧表差异：

- `skills` 额外包含 `source_json TEXT NOT NULL DEFAULT '{}'`。
- `operations` 记录 workflow 操作日志：`id`、`type`、`skill_name`、`status`、`message`、`created_at`。

兼容规则：

- Rust 新 migration 应以 Rust schema 为主。
- 读取既有 Node 数据时，Rust 不应因为旧列存在或旧表缺少新列而失败。
- Rust 写入 `skills` 和 `deployments` 时显式写 `updated_at`，兼容 Node MVP 中
  `updated_at TEXT NOT NULL` 但没有默认值的旧表。
- Rust 初始化 managed store 时会把 Node MVP `operations` 表迁移为 Rust operation
  schema；旧记录保留为 `legacy-node-<id>`，`status=ok` 映射为 `succeeded`，
  actor 标记为 `legacy-node`。
- 需要读取旧 `source_json` 时，应迁移到文件型 `source.json` 或明确的 Rust schema，而不是继续让 UI 直接解析 Node-only 列。

## 命名和版本规则

Skill name：

- 不能为空。
- 不能是 `.` 或 `..`。
- 不能包含 `/` 或 `\`。
- 应优先来自 `SKILL.md` frontmatter 的 `name` 字段；缺失时使用目录名。

版本目录：

- GitHub 安装版本使用 full commit SHA。
- Manual remote 使用 `manual-<contentHash12>`。
- Rollback 参数可以允许短 SHA 前缀匹配，但实际 `current` 必须指向完整版本目录名。

路径规则：

- 对用户可输入路径先展开 `~`，再做校验。
- 写入、部署、备份前应尽量使用规范化后的路径比较目标是否在预期根目录下。
- 不能用字符串拼接执行 shell；Git 和外部命令必须使用结构化参数。

Agent adapter 规则：

- `agent_id` 使用稳定小写标识，例如 `codex`、`claude`、`openclaw`、`cursor`、`claude-code`、`copilot`。
- adapter 必须声明它支持的 scan root、原生格式、部署模式和冲突策略。
- adapter 不应静默改写其它 agent 的 runtime 文件。
- adapter 之间不能共享未经声明的隐藏状态；共享状态只能通过 managed store 和 SQLite。

业务流程见 `docs/workflows.md`，模块拆分见 `docs/architecture.md`。
