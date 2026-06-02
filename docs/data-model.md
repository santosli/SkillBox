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
  adapters/
    <agent-id>/
  skillbox.sqlite
```

规则：

- `user-skills/<skill-name>` 保存用户创建或本地导入的 skill。
- `remote-skills/<skill-name>/versions/<version>` 保存远程或手动远程导入的不可变快照。
- `remote-skills/<skill-name>/current` 指向当前生效版本。
- `backups/imports` 保存从 runtime 目录迁移到 SkillBox 前的原始内容。
- `adapters/<agent-id>` 预留给 agent-specific cache、manifest 或转换产物；当前 Rust schema 尚未实现。
- 一个有效 skill 目录必须包含 `SKILL.md`。
- workspace 表记录 skills 所在工程目录或 runtime skills root，用于后续部署目标选择；workspace path 指向
  `.../.agents/skills`、`.../.codex/skills` 或 `.../.claude/skills` 这类 skills root，而不是单个 skill 目录。

当前实现仍以 `SKILL.md` 目录作为可读写单位。Claude、OpenClaw、Cursor、Claude Code、Copilot 等 agent 可能使用不同的原生文件格式；
支持这些格式时，应由 adapter 把原生格式映射到 SkillBox 的规范化记录，而不是让 UI 或 workflow 分别维护 schema。

## Remote Source Metadata

远程 skill 的来源元数据保存在 `remote-skills/<skill-name>/source.json`。

GitHub remote 使用这些字段：

```json
{
  "type": "github",
  "owner": "openai",
  "repo": "skills",
  "path": "skills/example",
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

skill_usage_events
  id TEXT PRIMARY KEY
  event_id TEXT
  skill_name TEXT NOT NULL
  agent_id TEXT NOT NULL
  runtime_root TEXT NOT NULL
  used_at TEXT NOT NULL
  recorded_at TEXT NOT NULL
  metadata_json TEXT NOT NULL DEFAULT '{}'

skill_usage_stats
  skill_name TEXT NOT NULL
  agent_id TEXT NOT NULL
  runtime_root TEXT NOT NULL
  usage_count INTEGER NOT NULL DEFAULT 0
  last_used_at TEXT NOT NULL
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
  PRIMARY KEY (skill_name, agent_id, runtime_root)
```

`workspaces.display_name` 由 path 推导：home-level global roots 使用 agent 名（例如 `Codex`、`Claude`），项目局部 roots 使用项目目录名（例如 `Pandora`）。`global` / `user` 不拼进名称，由 `kind` 字段表达。`imported_skill_count` 使用 import candidate 的同一套 imported 判定：内容 hash 已存在于 SkillBox managed store，或 workspace skill 已 symlink 到 managed root。

`operations` 记录会改变 managed store、runtime、SQLite、Git state 或偏好设置的动作。Rust core 统一写入，UI 只能读取展示或通过结构化命令触发新记录；记录从 UI 视角 append-only，MVP 不做自动清理。`payload_json` 保存操作细节，例如 from/to version、changed paths、backup path、affected deployments、commit SHA 或失败恢复状态。

`skill_usage_events` 记录真实 agent 调用事件，不记录 SkillBox 打开详情、部署、更新等管理行为。显式上报入口允许未导入 skill 写入；`event_id` 是可选幂等键，在同一 `agent_id + runtime_root` 下重复上报不会递增统计。`metadata_json` 只接受小型 JSON object，不保存 prompt、聊天正文、文件内容或 diff。`skill_usage_stats` 按 `skill_name + agent_id + runtime_root` 聚合，详情页和 skill card 按 skill name 汇总，workspace 页面按 runtime root 汇总。

usage hook 注入状态不写入 SQLite。SkillBox 设置页读取并更新各 agent 自己的 hook 配置文件：

- Codex App 和 Codex CLI：`~/.codex/hooks.json`，注入 `skillbox usage-hook codex` 到 `hooks.Stop`。
- Claude Code CLI：`~/.claude/settings.json`，注入 `skillbox usage-hook claude-code` 到 `hooks.Stop`。

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

- `kind=global` 表示 agent 自带或 home-level skills root，例如 `~/.codex/skills`、`~/.agents/skills`、`~/.claude/skills`。
- `kind=user` 表示用户项目局部 skills root，例如 `<project>/.agents/skills`。
- `source=auto` 表示由 scan 自动发现；`source=manual` 表示用户显式添加。
- 手动添加要求目录已存在；删除 manual workspace 只删除 registry 记录，不删除文件。
- `canonical_path` 用于去重，`path` 保留展示路径。

Node MVP 旧表差异：

- `skills` 额外包含 `source_json TEXT NOT NULL DEFAULT '{}'`。
- `operations` 记录 workflow 操作日志：`id`、`type`、`skill_name`、`status`、`message`、`created_at`。

兼容规则：

- Rust 新 migration 应以 Rust schema 为主。
- 读取既有 Node 数据时，Rust 不应因为旧列存在而失败。
- 需要读取旧 `source_json` 时，应迁移到文件型 `source.json` 或明确的 Rust schema，而不是继续让 UI 直接解析 Node-only 列。
- `operations` 是否保留应由后续 ADR 或 migration 决定；在决定前，不能把它当成所有工作流都依赖的唯一审计来源。

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
