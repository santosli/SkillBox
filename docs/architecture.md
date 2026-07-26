# SkillBox 架构地图

## 整体结构

SkillBox 是一个 Rust core + Tauri desktop monorepo。产品目标是管理跨 agent 的 skills、规则、提示词和能力包，
覆盖 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等主流 agent。

- `apps/desktop` 是 Tauri + React 桌面应用。
  - `src/App.jsx` 保留主 App 组件、状态和事件编排。
  - `src/components/` 按页面/领域聚合展示组件（dashboard、workspaces、rankings、history、settings、importReview、skillDetail、remoteSkills、userSkillsSync、common）。
  - `src/*.js` 是可独立测试的纯函数模块（如 `previewData.js`、`historyEntries.js`、`usageHooks.js`、`preferences.js`、`importFlow.js`、`skills.js`）。
- `apps/desktop/src-tauri` 是 Tauri command 层，负责把 UI 请求转发到 Rust crates。
- `crates/skillbox-core` 是核心业务 crate，当前实现扫描、导入、候选导入、GitHub install preview/apply、部署、SQLite 基础索引和偏好设置。
- `crates/skillbox-github` 负责 GitHub skill URL 解析和标准化。
- `crates/skillbox-git` 通过 `GitService` 负责 Rust 产品运行时的结构化 Git 调用和状态读取。
- `crates/skillbox-cli` 是 Rust CLI，和桌面应用共享同一套 Rust core。

新增业务能力必须进入 Rust crates。Node/npm 仅作为桌面前端、仓库脚本和测试运行时使用，不承载 SkillBox 产品业务逻辑。
App 自更新是桌面分发能力，边界在 Tauri updater plugin；React 只调用结构化 Tauri command，不直接下载或安装 release 资产。最近一次成功 updater metadata check 的展示快照保存在 managed SQLite preferences 中，用于跨启动的 24 小时节流和提醒恢复；该缓存不包含安装授权，点击 Update 后仍必须由 updater plugin 重新检查，并在下载安装时验证签名。

跨 agent 支持应通过 adapter 层表达：

- managed store 保存 SkillBox 的规范化状态，不绑定任何单一 agent。
- agent adapter 负责发现某类 runtime、读取该 agent 的原生格式、转换为 SkillBox 可管理的记录、并部署回该 agent 需要的路径或文件形态。
- 当前 Rust runtime-profile registry 先覆盖同一种 `SKILL.md` format 下的
  `.agents/.codex/.claude/.cursor` roots 和手动登记的 exact roots；不要把这当成
  对应 agent 原生格式的完整 adapter。
- runtime profile identity 与 usage/call `agent_id` 是两套 contract：前者描述部署
  target，后者描述本机观测事件来源，不能互相推断或替代。

## 调用关系

桌面应用调用链：

```text
React UI
  -> @tauri-apps/api/core invoke(...)
  -> apps/desktop/src-tauri/src/lib.rs Tauri command
  -> crates/skillbox-core / skillbox-github / skillbox-git::GitService
  -> 本地文件系统、SQLite、GitHub URL 解析、结构化 Git 命令
```

当前 Tauri commands：

- `managed_paths` -> `skillbox_core::managed_paths`
- `managed_state` -> `skillbox_core::managed_state`
- `managed_preferences` -> `skillbox_core::managed_preferences`
- `set_skip_local_import_confirmation` -> `skillbox_core::set_skip_local_import_confirmation`
- `list_skill_user_metadata` -> `skillbox_core::list_skill_user_metadata`
- `set_skill_user_metadata` -> `skillbox_core::set_skill_user_metadata`
- `migrate_legacy_skill_user_metadata` -> `skillbox_core::migrate_legacy_skill_user_metadata`
- `scan_skills` -> `skillbox_core::scan_skill_roots`
- `scan_import_candidates` -> `skillbox_core::scan_import_candidates`
- `scan_workspace_import_candidates` -> `skillbox_core::scan_import_candidates` scoped to one workspace root
- `import_candidates` -> `skillbox_core::import_candidates`
- `parse_github_url` -> `skillbox_github::parse_github_skill_url`
- `list_workspaces` -> `skillbox_core::list_workspaces`
- `scan_workspaces` -> `skillbox_core::scan_workspaces`
- `add_workspace` -> `skillbox_core::add_workspace`
- `preview_workspace_setup` -> `skillbox_core::preview_workspace_setup`
- `apply_workspace_setup` -> `skillbox_core::apply_workspace_setup`
- `forget_workspace` -> `skillbox_core::forget_workspace`
- `list_runtime_profiles` -> `skillbox_core::list_runtime_profiles`
- `preview_skill_deployment` -> `skillbox_core::preview_skill_deployment`
- `apply_skill_deployment` -> `skillbox_core::apply_skill_deployment`
- `find_remote_source_candidates` -> `skillbox_core::find_remote_source_candidates`
- `preview_remote_source_binding` -> `skillbox_core::preview_remote_source_binding`
- `bind_remote_source` -> `skillbox_core::bind_remote_source`
- `preview_github_remote_skill_install` -> `skillbox_core::preview_github_remote_skill_install`
- `install_github_remote_skill` -> `skillbox_core::install_github_remote_skill`
- `list_remote_skill_versions` -> `skillbox_core::list_remote_skill_versions`
- `preview_remote_version_change` -> `skillbox_core::preview_remote_version_change`
- `apply_remote_version_change` -> `skillbox_core::apply_remote_version_change`
- `list_operations` -> `skillbox_core::list_operations`
- `run_doctor` -> `skillbox_core::run_doctor`
- `list_history` -> `skillbox_core::list_history`
- `record_skill_usage` -> `skillbox_core::record_skill_usage`
- `list_skill_usage_rankings` -> `skillbox_core::list_skill_usage_rankings`
- `usage_audit` -> `skillbox_core::usage_audit`
- `preview_usage_skill_import` -> `skillbox_core::preview_usage_skill_import_for_source`
- `backfill_codex_session_usage` -> `skillbox_core::backfill_codex_session_usage`
- `backfill_claude_code_session_usage` -> `skillbox_core::backfill_claude_code_session_usage`
- `backfill_cursor_session_usage` -> `skillbox_core::backfill_cursor_session_usage`
- `usage_hook_statuses` -> `skillbox_core::usage_hook_statuses`
- `install_usage_hook` -> `skillbox_core::install_usage_hook`
- `check_app_update(force)` -> Tauri updater plugin HTTPS metadata check，非 force 请求复用 24 小时内、当前 app version 匹配的 SQLite 或进程内展示缓存
- `install_app_update` -> Tauri updater plugin signed download/install and restart；缺少进程内 pending update 时先重新检查，不能从缓存构造下载

Rust CLI 当前调用链：

```text
cargo run -p skillbox-cli --offline -- <command>
  -> crates/skillbox-cli
  -> skillbox-core / skillbox-github
```

## 模块边界

`skillbox-core` 的源码按领域模块组织，`lib.rs` 只保留共享常量、模块声明和 re-export：

- `types.rs` 公共数据结构与序列化类型
- `paths.rs` managed store 路径计算、初始化和 legacy 迁移
- `skills.rs` `SKILL.md` 解析、扫描、导入、symlink 部署
- `runtime_profiles.rs` versioned runtime profile registry、root precedence 和 capability policy
- `compatibility.rs` read-only frontmatter/target compatibility preview 与 stale-preview apply
- `import.rs` import candidates 扫描、类型推断、整目录重复候选分组、冲突与备份
- `state.rs` managed state 聚合与用户偏好
- `workspaces.rs` workspace registry 发现、注册与扫描
- `remote.rs` GitHub install preview/apply、remote source 绑定、update check、diff 预览、版本切换
- `marketplace.rs` Claude marketplace 候选搜索
- `git_sync.rs` user-skills Git 同步编排
- `usage.rs` usage 事件规范化、`confirmed/inferred/reference` evidence、单向升级与有界 provenance、Calls/stats、coverage、aggregate-only `usage-audit` 和 source-aware Import preview
- `usage_backfill.rs` 只从本机 Codex session rollout 的显式用户输入载体解析完整 `<skill>` 块或 `[$skill](.../SKILL.md)` 链接，作为逐回合 `inferred` invocation；忽略 catalog、普通 prose、assistant/tool/shell payload 与 output，按 turn + 规范化 name/path 去重，并用 session `cwd` 恢复 workspace identity
- `usage_backfill_claude.rs` 只从本机 Claude Code project JSONL 的原生 Skill tool/command attribution 恢复 `confirmed` 事件，解析真实 `SKILL.md`，不复制消息正文
- `usage_backfill_cursor.rs` 只读打开并验证 Cursor 本机 history SQLite schema；human bubble 中显式附加且解析到真实 `SKILL.md` 的 `context.cursorRules` 只记录为 `reference`，不兼容 schema fail closed
- `usage_backfill_cursor_transcripts.rs` 有界读取 Cursor agent transcript；只有 assistant `Read` / `ReadFile` tool use 的绝对路径通过 traversal、symlink、allowed-root、regular-file、大小和 `SKILL.md` 解析检查后，才记录 `confirmed` event
- `hooks.rs` agent hook 注入、transcript 解析，以及基于结构化 runtime context 的 workspace 归属
- `operations.rs` operation 与 history 记录
- `metadata.rs` 用户 favorites/tags 的 SQLite 持久化和 legacy desktop metadata 迁移
- `doctor.rs` managed store、SQLite、deployment、workspace 和 backup 的只读健康检查
- `db.rs` SQLite 打开、初始化、索引与偏好存取
- `fsutil.rs` 文件复制、symlink、哈希等底层工具
- `tests.rs` crate 级测试

`skillbox-core` 负责：

- skill 根目录扫描和 `SKILL.md` 读取。
- managed store 路径计算和初始化。
- workspace registry 的发现、手动添加、扫描统计和 forget 操作。
- runtime profile registry、structured frontmatter preservation 和部署 compatibility 判定。
- user/remote skill 导入。
- import candidates 扫描、类型推断、整目录快照去重和冲突检测。分组后的候选保留 primary `source_path` 与 `additional_source_paths`；React 展示全部来源，但只把 primary 作为结构化 import item 提交，避免一次 review 隐式创建多个不可单独 revert 的 active imports。
- preview-confirmed symlink 部署和部署索引。
- import backup 与 source 替换为 symlink。
- GitHub install preview/apply, GitHub-only remote source search, manual binding, update check, version listing, diff preview, update/rollback apply, and operation logging.
- SQLite schema migration、升级前备份、完整性校验、基础表和索引写入。
- 用户 favorites/tags 的 SQLite 持久化和桌面 legacy local-storage 迁移。
- managed store、deployment、workspace、import backup 和 metadata 的只读 Doctor 检查。
- 用户偏好读取与写入。
- skill usage 事件记录、evidence/provenance 升级、普通/System source identity、
  workspace-aware Calls/reference 聚合、aggregate-only diagnostics 和 agent hook 注入配置。
- 未来在 runtime profile 之上承载 native agent adapter 和跨 format 的规范化扫描/部署编排。

`skillbox-github` 负责：

- 接受 GitHub tree、blob、raw、contents API URL。
- 标准化 owner、repo、ref、path、repo URL 和展示 URL。
- 不负责下载、clone、稀疏 checkout 或版本历史写入。

`skillbox-git` 负责：

- 通过 `GitService` 作为 Rust 产品运行时唯一的 Git 服务边界。
- 用结构化参数执行 `git -C <repo> ...`，不拼接 shell 字符串。
- 读取仓库是否初始化、当前分支、dirty 状态和原始 status。
- 提供 init、origin 读取/设置、add、commit、push、`ls-remote` 等可复用 Git 原语。
- 集中处理 Git 网络命令的非交互环境变量、有界 timeout 和 stderr 返回。
- 不负责 managed store 级别的提交策略；`~/.skillbox/user-skills` 的同步编排在 `skillbox-core`。

repo-local 开发脚本可以保留少量自用 Git 调用，例如 Git hooks 安装；这些不是 SkillBox 产品运行时边界。

## 真相源和部署目标

`~/.skillbox` 是 SkillBox 管理状态的真相源：

```text
~/.skillbox/
  user-skills/
  remote-skills/
  backups/
  skillbox.sqlite
```

Runtime 目录只是部署目标：

- `~/.codex/skills`
- `~/.agents/skills`
- `~/.claude/skills`
- `~/.cursor/skills`
- 项目局部 `.codex/skills`
- 项目局部 `.agents/skills`
- 项目局部 `.claude/skills`
- 项目局部 `.cursor/skills`
- Claude、OpenClaw、Cursor、Claude Code、Copilot 等 agent adapter 声明的全局或项目局部 target

Workspace registry 记录这些 skills root，作为后续 deploy skills 的目标候选。`global` workspace 表示
home-level agent root，`user` workspace 表示项目局部 root；React 只展示和提交结构化请求，发现、分类、持久化和按 workspace 扫描 import candidates 都在 Rust core。

schema v6 让 workspace 显式记录 `profile_id`、`root_key` 和 `format`。旧
`agent_id` 暂时保留供数据库/API 兼容，但新的 workspace/deploy UI 不用路径字符串或
`agent_id` 推断 runtime identity。内建 precedence 依次为 Agents、Codex、Claude
Code、Cursor；它只决定 discovery/recommendation 顺序，不授权自动部署。

桌面 Add workspace 将普通项目目录交给 Rust core 做只读 preview，并只展示 core 返回的固定 root 候选。`Project` 是现有 `kind=user` 的 UI 语义标签；apply 会重放 preview 校验，再按 allowlist 创建至多一个项目局部 root。React 不拼接路径、不创建目录，Global scope 也不自动初始化目录。

不要在没有 adapter 语义的情况下猜测某个 agent 的目录布局。新增 agent 支持时，先定义 adapter 的发现路径、原生格式、部署方式和冲突处理。

默认部署方式是从 runtime 目录 symlink 到 managed store。部署前 Rust core 会严格解析
frontmatter，读取 profile capability，检查 target ownership，并返回
`compatible/warnings/blocked` 和 deterministic `preview_id`。apply 重新检查 skill
完整目录快照、target canonical path/state、profile/root/format 和 registry version；
任一变化都要求重新 preview。unknown optional frontmatter 只告警且原样保留，不自动
rewrite；runtime 目录中已有的非 symlink skill 不能被静默覆盖。

GitHub remote source 可以是仓库中的 skill 子目录，也可以是根目录包含 `SKILL.md` 的 standalone repository。后者在 metadata 中显式记录为 `root: true`，preview、install、update 和 deploy 共用同一份清理后的 repository worktree snapshot；Git checkout 的 `.git` metadata 不进入 managed store，逃逸 source root 的 symlink 在 copy 边界被拒绝。

重复候选只在名称、`SKILL.md` hash、推断类型、状态、冲突结果和完整导入快照均一致时合并；快照忽略顶层 `.git`，并覆盖其它路径、文件内容、Unix mode 与 symlink target。已 imported 的多个 runtime symlink 仅在解析到同一 managed `real_path` 时作为 alias 合并。primary 来源沿扫描 root 顺序选择；其它实体来源保留在 `additional_source_paths`，仅用于 review/search，不会在本次操作中被修改。导入只备份 primary、替换其 managed symlink 并写入 import record。仅 `SKILL.md` 相同但脚本、权限或资源不同的目录不能合并或复用；User 和 Remote 的已有 managed target 都必须通过完整快照校验。

## 当前状态与目标状态

当前状态：

- Rust core 已经是桌面应用的主要后端。
- Rust CLI 有 `init`、`version`、`paths`、`scan`、`parse-github-url`、
  `runtime-profiles`、`install-preview`、`install`、`import`、
  `deploy-preview`、preview-confirmed `deploy`、`user-skills-status`、
  `sync-user-skills`、`check-remote-updates`，并保留 `check-updates` 和
  `rollback` 兼容别名。
- Rust CLI 有 `remote-source-candidates`、`remote-source-preview`、`bind-remote-source`、`remote-versions`、`remote-preview-change`、`remote-apply-change`、`usage-record`、`usage-rankings`、`usage-audit`、各 provider history backfill、`usage-hook`、`usage-hook-status`、`usage-hook-install`、`doctor` 和 `operations`。
- Rust CLI 有 `workspaces`、`workspace-scan`、`workspace-add`、`workspace-forget` 来管理 workspace registry。
- Rust core 和 Tauri 已覆盖 `~/.skillbox/user-skills` 的共享 remote Git 同步。
- Rust core 已覆盖 remote skill 的 GitHub install preview/apply、GitHub update check、source binding、diff preview、update/rollback apply 和 operation log。
- Rust core 和 Tauri 已覆盖 usage stats 显式上报，以及 Codex App、Codex CLI、Claude Code CLI 的 Stop hook 注入入口。schema v7 把本机 evidence 分为 `confirmed`、`inferred` 和 `reference`；用户可见 `Calls` 只包含前两类，History references 单独展示。Rankings 支持 time range、User/Remote/System skill type、Agent 和 Workspace 的结构化过滤，并返回同一过滤快照内的 evidence totals、时间覆盖和可重叠 provenance source counts，以及 Codex、Claude Code、Cursor 最近一次 history scan 的文件/session 数。桌面 `Sync histories` 顺序调用三个 provider；单个 provider 失败不会撤销其他 provider 已成功写入或升级的幂等事件。
- Codex 本地 store 没有稳定的 provider-native skill-run total。Codex 结构化逐回合 skill carrier 只能作为 defensible `inferred` Calls；`usage-audit` 明确报告这个已知 undercount，不读取或返回聊天正文。
- 未来若接入 Codex reported runs，它属于独立的 provider-reported analytics 边界，必须携带 provider、subject kind、time window、scope 和 provenance；不得写入 `skill_usage_events`，也不得参与本地 ranking、total 或 delta。
- Tauri desktop 已覆盖 macOS app update check 和用户确认后的 install/restart；React 不直接处理 updater asset URL、签名或安装。
- SQLite schema 已由有序 transaction migrations 管理；已有数据库升级前生成一次一致性 backup，升级后执行 integrity check。
- Dashboard favorites/tags 已由 SQLite 持久化；桌面只在升级时读取一次 legacy local-storage metadata。
- Rust core、CLI、Tauri 和 Settings 已提供只读 Doctor workflow，并为主要 managed-store/runtime/Git/workspace/hook mutations 写 operation history。
- agent support 当前是 runtime-profile 管理的 `SKILL.md` roots，并包含 Codex、
  Claude Code、Cursor 的受限历史观测 provider；尚未覆盖 Claude、OpenClaw、
  Cursor、Claude Code、Copilot 的完整原生格式和部署语义。
- legacy Node CLI/core 已移除；旧 Node MVP 写入的 managed store 目录和 `source.json` 字段仍按兼容规则读取。

目标状态：

- UI 和 CLI 都只通过 Rust core 执行业务逻辑。
- 增加 agent adapter registry，让 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等 runtime 通过同一 managed store 管理。

本文件不记录逐步操作和字段细节；workflow 看 `docs/workflows.md`，存储字段看 `docs/data-model.md`。
