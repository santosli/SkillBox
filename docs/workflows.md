# SkillBox Workflows

本文件定义工作流入口、步骤、失败处理和完成标准。实现位置和长期目标见 `docs/architecture.md`。
SkillBox 的目标是跨 agent 管理，不只覆盖 Codex。当前 workflow 以 Rust runtime
profiles 管理 `SKILL.md` roots；
Claude、OpenClaw、Cursor、Claude Code、Copilot 等需要通过 agent adapter 扩展。

## 1. Scan Local Skill Roots

触发条件：

- UI 刷新 managed state 或扫描 import candidates。
- Rust CLI 执行 `scan`。

步骤：

- 从 versioned Rust profile registry 按 precedence 读取
  `~/.agents/skills`、`~/.codex/skills`、`~/.claude/skills`、
  `~/.cursor/skills` 及对应项目局部 roots。
- 后续通过 agent adapter 读取 Claude、OpenClaw、Cursor、Claude Code、Copilot 等 runtime roots。
- 在每个 root 内递归查找包含 `SKILL.md` 的目录。
- 扫描摘要读取 frontmatter 的 `name`、`description`、`version`；严格的完整
  structured parse 在 deployment compatibility preview 执行。
- 计算 `SKILL.md` content hash。
- 标记 source root、是否 symlink、real path。
- 扫描 import candidates 时把存在且可读取的 skills root 写入 `workspaces` registry；home-level roots 记为 `global`，项目局部 roots 记为 `user`。
- 按 skill name 排序返回，同时保留 scan errors。

失败与回滚：

- 不存在的 root 跳过。
- 单个 skill 读取失败时记录 error，不中断整个扫描。
- scan 不应写入 runtime 目录，因此不需要回滚。

完成验证：

- `cargo test --offline`
- `npm test`
- `cargo run -p skillbox-cli --offline -- scan ~/.codex/skills ~/.agents/skills`

## 2. Import Existing Skills

触发条件：

- UI first-use 或用户主动扫描本机已有 skills。
- Rust CLI 执行 `import`。

步骤：

- 扫描 import candidates。
- 根据路径和内容推断类型：当前 `.agents/skills` 倾向 user，`.codex/skills` 倾向 remote，`.system` 默认不选中，包含 GitHub 来源信息的未知目录倾向 remote。
- 对名称、`SKILL.md` hash、推断类型、状态、冲突结果以及完整导入快照都一致的实体副本进行分组；快照忽略顶层 `.git`，但覆盖其它文件、Unix mode、目录和 symlink。仅 `SKILL.md` 相同而脚本、权限或资源不同的候选保持分离。
- 已 imported 的多个 runtime symlink 只有解析到同一个 managed `real_path` 时才作为 alias 合并。
- 分组候选按扫描 root 顺序保留一个 primary `source_path`，并通过 `additional_source_paths` 保留其它实体来源。Import Review 显示来源数量和可展开路径，搜索会匹配任一来源，并明确其它副本在本次操作中不会改变。
- agent adapter 引入后，候选项还应携带 `agent_id`、原生格式和 target scope。
- 检查 managed target 是否冲突。
- user skill 复制到 `~/.skillbox/user-skills/<name>`。
- 导入分组候选时，只对 primary source 执行备份、symlink 部署和 import-record 写入；additional sources 保持原状，避免隐式创建多个无法单独 revert 的 active imports。
- remote skill 复制到 `~/.skillbox/remote-skills/<name>/versions/manual-<contentHash12>`，并更新 `current` symlink。
- 如果用户选择 deploy back to source，先把原 runtime 目录移动到 `~/.skillbox/backups/imports/<name>-<contentHash12>`，再在原位置创建指向 managed target 的 symlink。
- deploy back 是对已 review、完整 snapshot 相同的导入来源执行 ownership transfer，并通过 backup/import record 支持保守 revert；它不是普通 workspace deployment，不运行 runtime-profile compatibility preview。普通 deploy 和 GitHub install-to-target 仍必须通过 compatibility preview。
- 写入 SQLite `skills`，必要时写入 `deployments`。
- deploy back 成功后，为每个 imported skill 写一条 `import_records` active 记录，保存 source path、managed target、backup path 和 content hash，供后续 revert 使用。
- 扫描 import candidates 时，只有 runtime skill 是指向 SkillBox managed root 的 symlink 时才显示为 imported；仅 content hash 已存在于 managed store 不代表该 runtime 位置仍被 SkillBox 管理。

失败与回滚：

- User 或 Remote managed target 已存在但完整导入快照不一致时拒绝；不能只依赖 `SKILL.md` hash 判断整个 skill 相同。
- 原 runtime 位置是指向其它位置的 symlink 时拒绝。
- deploy back to source 创建 symlink 失败时，应把 backup rename 回原位置。
- 不覆盖用户内容，不删除 backup。
- `import_records` 写入失败时应把失败返回给调用方，不把该 import 显示为可自动 revert。

完成验证：

- `cargo test --offline`
- `npm test`
- 使用临时目录运行 Rust CLI：`cargo run -p skillbox-cli --offline -- import <source-dir> --type user --managed-root <temp-skillbox-root>`
- UI 路径变更时，手动验证 import review 与本地 import 确认弹窗中的 User/Remote 类型选择、冲突、默认选中和备份提示。
- 使用两个 runtime roots 验证导入内容一致的副本只显示一行、列出两个位置、只为 primary 创建 backup/symlink，并保持 additional source 不变；修改任一附加脚本或 executable bit 后应恢复为两条候选。

## 3. Revert Local Import

触发条件：

- Rust CLI 执行 `import-records [--skill <name>]` 查看可恢复记录。
- Rust CLI 执行 `revert-import <import-record-id>`。
- Tauri command：`list_import_records`、`revert_import`。
- 桌面详情页在 deployment/workspace 区域显示 `Revert import`。

步骤：

- `list_import_records` 读取 active/reverted import records，并按需从旧 `deployments` + `backups/imports` 做保守 legacy reconciliation。
- 只有证据链唯一且安全的 legacy import 才会写入 `legacy=true` active record；歧义 backup 或同 skill 多 workspace deployment 不自动生成可 revert 记录。
- `revert_import` 只接受 import record id，不接受 skill name 或 source path。
- 执行前复验 record 为 active、backup 存在且 `SKILL.md` name/hash 匹配、source path 是指向记录 managed target 的 symlink 或 source path 不存在。
- 如果同一 managed skill 有多个 workspace deployment 或多个 active import record，拒绝 revert，避免产生多个 source。
- 删除 source symlink 后，把 backup rename 回 source path；如果 rename 失败，尝试重新创建 source symlink 并保持 record active。
- 删除对应 deployment 记录，标记 import record 为 `reverted`，并记录 `revert_import` operation。
- remote skill revert 保留 `remote-skills/<name>/versions`、`current` 和 `source.json`。
- user skill revert 在无其它引用时删除 `user-skills/<name>` managed copy。

失败与回滚：

- source path 是非 symlink、symlink 指向其它位置、backup 缺失或 backup 内容不匹配时拒绝。
- 多 workspace deployment 时拒绝，不做 partial revert。
- 文件系统恢复成功但 SQLite 更新失败时，不反向覆盖已恢复的用户目录；后续 list 应显示状态不一致或错误。

完成验证：

- `cargo test -p skillbox-core --offline import`
- `cargo run -p skillbox-cli --offline -- import-records --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- revert-import <record-id> --managed-root <temp-skillbox-root>`
- 桌面 UI 手动验证 warning 入口、danger 确认按钮、blocked reason，以及 revert 后 runtime path 是真实目录。

## 4. GitHub Install

触发条件：

- Rust CLI 入口：`skillbox install-preview <github-url>` 先生成 diff preview，
  `skillbox install <github-url> --preview-id <id> [--target <path>]` 确认后安装。
- Rust core API：`preview_github_remote_skill_install` + `install_github_remote_skill`。
- Desktop UI `Install` dialog accepts GitHub tree/blob/raw/API URLs, opens a diff
  review first, and only calls the Rust install API after confirmation.

步骤：

- 解析 GitHub repository、tree、blob、raw 或 contents API URL；standalone repository URL 和仓库根 `SKILL.md` URL 会显式标准化为 repository-root source。
- 标准化 owner、repo、ref、path、root、repoUrl、url。目录 source 使用非空 `path`；repository-root source 使用空 `path` 和 `root: true`。
- Preview 阶段对目录 source 使用 `skillbox-git::GitService::fetch_ref_path` 拉取指定 ref/path；对 repository-root source 使用 `fetch_ref_tree` 拉取完整 worktree，并在生成 diff 前移除 `.git` checkout metadata。
- Preview 阶段验证下载目录包含 `SKILL.md`，读取 skill name 并校验命名。
- Preview 阶段生成 empty directory -> remote skill directory 的全文件 diff 和 deterministic `preview_id`。
- Preview 阶段不得写入 `remote-skills/<name>`、`current`、`source.json`、SQLite，也不得部署到 runtime。
- Confirm/install 阶段重新解析和拉取 GitHub 来源，并验证 `preview_id` 与 URL、ref、resolved SHA、skill name、target root 匹配。
- 写入 `remote-skills/<name>/versions/<installedSha>`。
- 更新 `remote-skills/<name>/current` symlink。
- 写入 `source.json`，包含 GitHub 来源和 `installedSha`、`latestSha`。
- 写入 SQLite `skills`。
- 如果提供 target，它必须是已登记 workspace；install preview 同时返回 runtime
  compatibility，confirm/install 会重新计算 compatibility 和 install preview identity
  后才执行 deploy workflow。
- Desktop UI import does not provide target by default, so a newly installed
  GitHub skill remains in the managed store until the user explicitly deploys it.

失败与回滚：

- URL 不指向含 `SKILL.md` 的 skill 目录或 standalone repository root 时拒绝。
- repository-root source 的 preview、version snapshot 和 runtime deployment 只使用清理后的 worktree；`.git` 和其它 checkout metadata 不进入 managed store。
- repository-root worktree 中逃逸 source root 的 symlink 会在 copy 前拒绝，不写 managed store。
- `install` 缺少 `preview_id` 或 preview 身份已过期时拒绝，不写 managed store。
- Git 命令失败时清理临时目录，不写 managed store。
- version 已存在时可以复用，但仍需验证 `SKILL.md`。
- target 部署失败时保留已安装版本，并把 deployment error 返回给调用方。

完成验证：

- URL parse：`cargo run -p skillbox-cli --offline -- parse-github-url <github-url>`
- Rust preview：`cargo run -p skillbox-cli --offline -- install-preview <github-url> --managed-root <temp-skillbox-root>`
- Rust install：`cargo run -p skillbox-cli --offline -- install <github-url> --preview-id <id> --managed-root <temp-skillbox-root>`
- `cargo test -p skillbox-core --offline install_github_remote_skill`

## 5. Preview And Deploy Managed Skill

触发条件：

- Rust CLI 先执行 `deploy-preview <skill-name> --target <path>`，再把返回的
  `preview_id` 传给 `deploy <skill-name> --target <path> --preview-id <id>`；
  warning 还需要 `--confirm-warnings`。
- Rust CLI 执行 `undeploy <skill-name> --target <path>`。
- 桌面详情页打开 Deploy workspace 弹窗，勾选 workspace 执行 deploy，取消已勾选 workspace 执行单 workspace remove/undeploy。
- import workflow 的 deploy back to source 属于上文 Import Existing Skills 定义的 reviewed ownership transfer，不走本节的 compatibility preview。

步骤：

- target 必须是 workspace registry 中已存在、可读且 canonical path 匹配的 root。
- Rust 从 workspace 的 `profile_id/root_key/format` 读取 runtime identity，不从
  React path string 或 usage `agent_id` 推断。
- read-only preview 严格解析 `SKILL.md` frontmatter，并按 profile capability 返回
  `compatible`、`warnings` 或 `blocked`、结构化 issues 和 `preview_id`。
- unknown optional frontmatter 作为 warning 原样保留；SkillBox 不 rewrite、translate
  或删除字段。
- malformed frontmatter、name mismatch、profile/root/format mismatch、unsupported
  deployment mode、unsafe target、既有非 symlink/foreign symlink 会 blocked。
- Desktop 对每个新勾选 target 请求 Rust preview：blocked 不可部署，warning 需要显式
  确认，compatible 可直接确认。
- apply 重算 skill 全目录 snapshot、target canonical path/state、profile identity、
  registry version 和 frontmatter compatibility；stale preview 拒绝。
- target 不存在时，仅在显式确认后创建指向 managed skill 的 symlink。
- target 是 symlink 且已指向同一 managed path 时视为成功。
- 写入 SQLite `deployments`。
- undeploy 时只删除 `target_root/<skill-name>` 这个 symlink，并删除 SQLite `deployments` 对应记录。
- 桌面执行 undeploy 前必须显示明确提醒，用户确认后才应用取消勾选的 workspace。

失败与回滚：

- target 是非 symlink 时拒绝。
- target 是 symlink 但指向其它位置时拒绝。
- 创建 symlink 失败时不写 deployment 记录。
- undeploy 遇到非 symlink 或指向其它位置的 symlink 时拒绝，不能删除磁盘内容。
- active import 的 source workspace 必须通过 Revert Import 恢复，不能直接 undeploy；同一 skill 的其它 workspace deployment 仍可单独移除。
- 不删除非 SkillBox 管理的内容。
- preview 不写 runtime；apply 前 skill、target 或 workspace profile metadata 变化时
  要求重新 preview。

完成验证：

- `cargo test --offline`
- `npm test`
- `cargo run -p skillbox-cli --offline -- runtime-profiles`
- `cargo run -p skillbox-cli --offline -- deploy-preview <skill-name> --target <registered-runtime> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- deploy <skill-name> --target <registered-runtime> --preview-id <id> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- undeploy <skill-name> --target <temp-runtime> --managed-root <temp-skillbox-root>`
- 检查 target path 是 symlink，real path 指向 managed store。
- 检查 undeploy 后 target symlink 消失，非 symlink target 不会被删除。

## 5.1 Delete Managed Skill

触发条件：

- Rust CLI 先执行 `delete-preview <skill-name>`，再将返回的 `preview_id` 传给 `delete <skill-name> --preview-id <id> --confirm <skill-name>`。
- 桌面 Skill Detail 的 Danger zone 打开删除预览，输入完整 skill name 后确认。

步骤：

- 预检 managed user 目录或完整 remote skill root、所有 SQLite deployment 与已注册 workspace 中推断出的 symlink。
- active import、非 symlink runtime target、指向其它位置的 symlink 或不安全 managed path 都会阻断整个操作，预检失败时不修改文件或数据库。
- apply 时重新生成并校验 preview identity；user skill 绑定完整目录快照，remote skill 绑定完整 remote root（包括全部 versions、`source.json` 和 `current` link），避免确认后状态变化。
- 将 managed user skill 或完整 remote skill root 原子移动到 `backups/deletions`。
- 删除所有已确认归 SkillBox 管理的 workspace symlink；workspace 注册本身及其它 skills 保持不变。
- 在单个 SQLite transaction 中删除 active skill index、deployments、favorites/tags，并从 remote update cache 剔除该 skill。
- operation、usage history、reverted/failed import history 与 recovery backup 保留。
- remote root 即使缺少或损坏 `current`/`versions` 仍可通过 core/CLI 预览后整根删除；remote root 本身若是 symlink 或非目录仍会拒绝。
- remote update cache 是可丢弃的派生状态；损坏时删除 cache row，不阻断 skill 删除。

失败与回滚：

- 文件或 SQLite 清理失败时，将 managed skill 从 deletion backup 恢复，并重建本次已移除的 symlink。
- preview identity 变化时拒绝 apply，要求重新预览。
- active import source workspace 使用 canonical/normalized path 判断，不能通过 symlink parent 或相对路径绕过 Revert Import 要求。
- workspace target 会先原子移动到同目录 quarantine 再校验归属；并发替换出的未知内容优先迁移到 `backups/deletion-conflicts`，无法迁移时保留原 quarantine 并由 Doctor 报告，绝不自动删除。
- 删除不会自动 commit 或 push user-skills Git repository；user skill 删除会作为普通 Git deletion 留给用户后续 review/sync。

完成验证：

- `cargo test -p skillbox-core --offline delete_skill`
- `cargo run -p skillbox-cli --offline -- delete-preview <skill-name> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- delete <skill-name> --preview-id <id> --confirm <skill-name> --managed-root <temp-skillbox-root>`
- 检查 managed skill 已移入 `backups/deletions`、全部关联 symlink 消失、workspace registry 和历史记录保留。

## 6. Check Remote Updates

触发条件：

- Rust CLI 当前入口：`cargo run -p skillbox-cli --offline -- check-remote-updates [skill-name] [--managed-root <temp-skillbox-root>]`。
- Rust CLI 兼容别名：`cargo run -p skillbox-cli --offline -- check-updates [skill-name] [--managed-root <temp-skillbox-root>]`。
- Tauri command：`check_remote_skill_updates`。
- 桌面启动只调用 `cached_remote_skill_updates` 读取上一次检查结果，不主动查询远端。

步骤：

- Dashboard `Refresh status` 遍历所有 `remote-skills/<name>/source.json`；remote skill detail 的 `Check update` 只检查当前 skill。
- 只处理 `type: github` 的 remote skill。
- `refKind: tag`、`refKind: commit` 或 `tracking: false` 的 GitHub source 标记为 `pinned`，不执行远端更新判断。
- 对 tracking branch 使用 `git ls-remote <repoUrl> <ref>` 查询最新 SHA。
- `git ls-remote` 必须以非交互方式执行，并设置有界超时；默认超时为 30 秒，用户可在 Settings 调整。
- 全量 remote update check 必须限制并发，当前上限为 3，避免多个慢 Git 连接拖住整个 app。
- 优先比较 latest remote SHA 与 `currentVersion`；没有 `currentVersion` 时兼容比较 `installedSha`。
- 返回每个 remote skill 的 `skillName`、`sourceType`、`currentVersion`、`installedSha`、`latestSha`、`refKind`、`tracking`、`updateAvailable`、`state`、`message`。
- 成功执行远端检查后，把完整检查结果和检查时间缓存到 managed SQLite preferences；下次桌面启动复用缓存状态，只有用户刷新或自动刷新后才更新缓存。
- 如果某个 skill 上一次检测成功，本次远端检测超时或 Git 失败时保留上一次成功状态，只在 `message` 中记录 `Last check failed`。
- 读取缓存时仍会基于当前本地 `remote-skills/<name>/source.json` 判定缺失 source 的 skill，并显示为 `No source`，避免把未绑定 source 的 remote skill 显示为未检查。
- Dashboard 的 `Refresh status` 通过 Tauri command 刷新 user-skills Git 状态和 remote update check，再把行状态更新为 `Needs sync`、`Synced`、`Update available`、`Up to date`、`Pinned`、`No source`、`Check failed` 或 `Not checkable`。
- Dashboard 的 `Checked` 列显示最近一次 status check 的时间；未检查前显示 `not checked`。
- 桌面 UI 默认每 5 分钟自动执行一次 status check，间隔通过 Settings 的 `Status refresh` 设置保存到 managed preferences。

失败与回滚：

- 缺失 `source.json` 的 remote skill 标记为 `no_source`，提示用户先绑定 GitHub source。
- 非 GitHub remote 标记为 `not_checkable`。
- 网络或 Git 失败应作为该 skill 的 update check error 返回，不应破坏现有版本。
- 这个 workflow 只检查状态，不更新 `source.json`、`current` symlink 或版本目录。

完成验证：

- `cargo test -p skillbox-core --offline check_remote_skill_updates`
- `cargo run -p skillbox-cli --offline -- check-remote-updates --managed-root <temp-skillbox-root>`
- `npm test`
- 桌面 UI 视觉验证 Dashboard `Refresh` 按钮、Checked 时间、状态 badge、Available updates 计数、notice，以及 Settings 中的自动刷新间隔。

## 7. Bind Remote Source

触发条件：

- Rust CLI 当前入口：`remote-source-candidates`、`remote-source-preview`、`bind-remote-source`。
- Tauri command：`find_remote_source_candidates`、`preview_remote_source_binding`、`bind_remote_source`。
- 桌面 `Bind source` 弹窗打开时会后台调用 `find_remote_source_candidates`，候选只用于预览，仍需用户确认后才绑定。
- 用户为已有 remote skill 手动添加 GitHub source URL。
- 用户触发 Claude Marketplace candidate search，为已有 remote skill 自动寻找可能的 source。
- 接受 GitHub skill directory URL、目录内 `SKILL.md` URL，以及根目录含 `SKILL.md` 的 standalone repository URL / root `SKILL.md` URL。repository-root source 使用清理后的完整 worktree，且不保存 `.git` metadata。

步骤：

- 自动搜索调用 `https://claudemarketplaces.com/api/skills` 拉取 Claude Marketplace skills 列表，本地按 skill name 精确命中优先过滤；没有精确命中时再退到 name/path contains。
- 桌面自动搜索必须先渲染弹窗和后台搜索提示；搜索期间用户仍可手动粘贴 URL 或关闭弹窗。
- 自动搜索把 marketplace 结果映射回 GitHub source URL，结果按 skill name、path、marketplace install signal 和 stars 排序。
- 自动搜索只返回候选、score 和 match reasons，不写 `source.json`，不修改版本目录，必须由用户确认后继续绑定。
- 绑定前校验会先尝试候选 URL 的原始 path；若 marketplace path 是逻辑 skill 名称而不是仓库真实目录，继续尝试 `skills/<name>`、`skills/public/<name>`、`.claude/skills/<name>` 等常见布局，并把成功解析出的 GitHub URL 写入预览和 `source.json`。
- 桌面 source preview / bind command 必须在线程池中执行；Git fetch 必须非交互且有界超时，避免 `Checking source...` 阻塞整个 app。
- 校验本地 skill name，并解析 GitHub URL 的 owner、repo、ref 和 path。
- 在临时工作树中 fetch 目标 ref。目录 source 只 checkout URL 指向的 skill path；repository-root source checkout 完整 worktree 并移除 `.git` metadata。
- 读取远端 `SKILL.md`，和本地 `current` 指向的 skill 做本地验证。
- `exact_match`：远端 skill name 和内容 hash 都匹配，可以绑定 source。
- `same_skill_changed`：远端 skill name 匹配但内容 hash 不同，可以绑定 source，但必须告知用户当前内容不会被替换。
- `mismatch`：远端 skill name 与本地 skill name 不一致，拒绝绑定。
- 对 `exact_match` 和 `same_skill_changed` 写入 `remote-skills/<name>/source.json`，包含 GitHub 来源、`refKind`、`tracking`、`currentVersion`、`installedSha`、`latestSha`。
- `same_skill_changed` 不写入 `versions/<latestSha>`，不切换 `current`，不 redeploy runtime。
- 所有 bind 执行都记录 `bind_remote_source` operation；成功、失败和 mismatch 拒绝都必须有最终状态。

失败与回滚：

- Git fetch、路径 checkout、`SKILL.md` 读取或 metadata 写入失败时，不改变 `current` 和版本目录。
- mismatch 拒绝不会写 `source.json`。

完成验证：

- `cargo test -p skillbox-core --offline source_binding`
- `cargo run -p skillbox-cli --offline -- remote-source-candidates <skill-name> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- remote-source-preview <skill-name> <github-url> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- bind-remote-source <skill-name> <github-url> --managed-root <temp-skillbox-root>`
- 桌面 UI 手动验证 source binding dialog：`exact_match` 可绑定，`same_skill_changed` 明确提示当前版本不会被替换，`mismatch` 禁用绑定。

## 8. Update Remote Skill

触发条件：

- Rust CLI 当前入口：`remote-versions`、`remote-preview-change --action update`、`remote-apply-change --action update`。
- Tauri command：`list_remote_skill_versions`、`preview_remote_version_change`、`apply_remote_version_change`。
- 桌面 UI：remote skill detail 中的 `Review update` 打开 diff review dialog，用户确认后调用 apply。
- GitHub source 必须已经绑定，并且 update check 已取得 `latestSha`。

步骤：

- 先执行 check updates。
- 如果没有新 SHA，返回 no-op。
- 桌面打开 review dialog 后必须先渲染 loading 状态，再启动 `preview_remote_version_change`。
- 预览阶段先列出 `versions/*`，标记当前 `currentVersion`。
- 在临时工作树中 fetch 目标 ref。目录 source checkout `source.json.path` 对应的 skill 目录；`source.json.root: true` 的 repository-root source 使用移除 `.git` metadata 的完整 worktree。
- 验证 `SKILL.md` 和 skill name。
- 应用前对当前 `current` 目录和目标 snapshot 生成 no-index diff；diff 必须包含所有新增、修改、删除文件，路径规范化为 skill 内相对路径。
- diff preview 对二进制文件或超过 1 MB 的文件保留文件行、hash 和 size，但不展开文本 diff。
- 如果 source revision 已变化但 skill 文件内容没有变化，diff review 必须明确显示 no file changes，并允许用户确认以记录最新 revision。
- apply 阶段写入 `versions/<latestSha>`；如果目录已存在，则复用并重新验证。
- apply 阶段更新 `current` symlink。
- apply 阶段更新 `source.json.currentVersion`；当目标版本是 GitHub commit SHA 时同步 `installedSha`。
- 记录 SQLite skill hash/path 状态。
- 永久保留旧版本目录，供 rollback 使用。

失败与回滚：

- 下载失败不改变 `current`。
- 新版本无效时拒绝更新，并保留旧版本。
- `current` symlink 切换后的 metadata/index 写入失败必须尝试恢复到旧 `current`，并在错误中说明恢复结果。
- 不删除旧版本目录。

完成验证：

- `cargo test -p skillbox-core --offline apply_`
- `cargo run -p skillbox-cli --offline -- remote-versions <skill-name> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- remote-preview-change <skill-name> --action update --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- remote-apply-change <skill-name> --action update --to <sha> --managed-root <temp-skillbox-root>`
- 手动验证：安装一个固定旧 ref 后更新到新 ref，确认 `current` 指向新 SHA。
- 桌面 UI 手动验证：update review 打开期间显示 loading，完成后展示所有变更文件，文本文件展示 unified diff，二进制或大文件展示 hash/size metadata，no-file-change 更新显示明确说明，确认后刷新版本列表和 operation history。
- Tauri 验证：`preview_remote_version_change` 这类 Git/diff 预览 command 必须放到 blocking worker，避免点击 `Review update` 时阻塞窗口渲染。

## 9. Rollback Remote Skill

触发条件：

- Rust CLI 当前入口：`remote-versions`、`remote-preview-change --action rollback`、`remote-apply-change --action rollback`。
- Rust CLI 兼容别名：`rollback <skill-name> --to <sha>`。
- Tauri command：`list_remote_skill_versions`、`preview_remote_version_change`、`apply_remote_version_change`。
- 桌面 UI：remote skill detail 的 version list 对非当前版本显示 `Rollback`，复用 update 的 diff review dialog。

步骤：

- 校验 skill name。
- 预览阶段先列出 `versions/*`，标记当前 `currentVersion`。
- 在 `remote-skills/<name>/versions` 查找等于 rollback 参数或以该参数开头的版本目录。
- 验证目标版本包含 `SKILL.md`。
- 应用前对当前版本和目标版本生成 no-index diff；diff 必须展示所有受影响文件，包括回滚后会删除的文件。
- 更新 `current` symlink 指向目标版本。
- 如果存在 `source.json`，更新 `currentVersion`；当目标版本不是 GitHub commit SHA 时将 `installedSha` 置空。
- 更新必要的 SQLite 状态。

失败与回滚：

- 找不到版本时拒绝。
- 短 SHA 匹配多个版本时应拒绝。
- `current` symlink 切换后的 metadata/index 写入失败必须尝试恢复到原 `current`。
- 不删除任何 version 目录。

完成验证：

- `cargo test -p skillbox-core --offline remote_version`
- `cargo test -p skillbox-core --offline apply_`
- `cargo run -p skillbox-cli --offline -- remote-preview-change <skill-name> --action rollback --to <sha-or-prefix> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- remote-apply-change <skill-name> --action rollback --to <sha-or-prefix> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- rollback <skill-name> --to <sha-or-prefix> --managed-root <temp-skillbox-root>`
- 桌面 UI 手动验证：rollback review 展示回滚后会新增、修改、删除的所有文件，确认后 `current` 和版本列表同步刷新。

## 10. Operation Log

触发条件：

- Rust core 执行会改变 managed store、runtime、SQLite、Git state 或偏好设置的动作。
- 当前 direct/reviewed import、deploy、undeploy、skill type change、import revert、remote install/source bind/update/rollback、workspace add/forget、user-skills Git remote/sync 和 usage hook injection 必须写 operation log。
- Rust CLI 入口：`operations`。
- Tauri command：`list_operations`。
- Tauri command：`list_history`。
- 桌面 UI：remote skill detail 默认折叠最近的 skill operation history，只显示日志入口和事件数；展开后每条记录显示完成时间，未完成时显示开始时间。左侧 History 页展示全局 skill usage events 和 SkillBox operation logs 的合并时间线，并支持按 Skill calls / Operations 过滤。

步骤：

- 操作开始时写入 `started` record，包含 operation type、actor、entity type/name、started time、summary 和 payload。
- 操作成功时更新为 `succeeded`，写入 finished time 和最终 payload。
- 操作失败、验证拒绝或恢复失败时更新为 `failed`，写入 finished time、error 和恢复相关 payload。
- 记录由 Rust core append/update；React 只能读取展示，不能编辑、删除或伪造记录。
- MVP 永久保留 operation log，不自动清理。
- favorites/tags、自动 cache/index refresh、scan 和纯读取 workflow 不写 operation log，避免 History 被低风险状态刷新淹没。

失败与回滚：

- 业务操作失败时必须尽量把对应 operation 标记为 `failed`。
- operation 写入失败不能静默吞掉；调用方应收到错误或包含日志失败说明的结果。
- UI 无法加载 operation history 时，只在该 skill 的操作区展示加载失败，不阻断其它 skill 管理能力。

完成验证：

- `cargo test -p skillbox-core --offline operation`
- `cargo run -p skillbox-cli --offline -- operations --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- operations --entity-type skill --entity-name <skill-name> --managed-root <temp-skillbox-root>`
- 桌面 UI 手动验证 remote skill detail 中成功和失败 operation 都可见，并验证 History 页能同时显示 skill calls 和 operations。

## 11. Sync User-Skills Git

### Outbound Commit And Push

触发条件：

- Rust CLI 入口：`skillbox sync-user-skills [--remote <git-url>] [--message <msg>] [--no-push]`。
- Rust CLI 状态入口：`skillbox user-skills-status`。
- Tauri command：`user_skills_git_status`、`user_skills_git_changes`、`set_user_skills_git_remote` 和 `sync_user_skills_git`。

步骤：

- 确保 `~/.skillbox/user-skills` 存在。
- 默认所有本地 user skills 通过同一个 `~/.skillbox/user-skills` Git 仓库和同一个 `origin` remote 同步。
- 如果没有 `.git`，初始化 `main` 分支 Git 仓库。
- Settings 中配置 shared `origin` remote；commit review dialog 只读展示当前 remote，不直接修改 remote。
- 桌面 UI 的 sync action 必须先打开 commit review dialog：展示 changed files、当前 diff、可编辑 commit message、只读 remote URL、push 选项，并允许用户选择本次提交的文件。
- commit review dialog 默认根据选中文件生成 Conventional Commit message；用户手动编辑后不再因勾选变化覆盖，除非主动重新生成。
- 没有 changed files 或没有选中文件时，commit action 必须禁用；提交过程中必须展示 loading/progress 状态，避免用户误以为界面卡住。
- Rust core 通过 `user_skills_git_changes` 返回结构化 changed files 和 diff；React 只展示和收集选择，不直接读取文件系统或执行 Git。
- Rust core 通过 `user_skills_git_status.changed_paths` 返回 dirty 文件路径；Dashboard 行状态必须按 skill 目录细分，只有包含 changed path 的 user skill 显示 `Needs sync`，其他 user skill 保持 `Synced` 或对应全局配置状态。
- CLI 或未提供文件选择时执行 `git add .`；桌面 UI 提供 `selected_paths` 时只 add 这些经过校验的相对路径。
- 如果有 staged 变更，使用提供的 commit message 创建 commit；message 为空时默认 `Sync user skills`。
- 默认 push 到 `origin main` 并设置 upstream；Rust CLI 可用 `--no-push` 跳过 push。
- 返回 initialized、remote_updated、branch、dirty、raw_status、committed、commit_sha、pushed、push_attempted、state、message。

冲突策略：

- User-skills sync 是 commit + optional push workflow。SkillBox 不会在同步中执行 `git pull`、`git merge` 或 `git rebase`，也不会创建或解析 merge conflict markers。
- SkillBox 不对 user skills 使用 last-write-wins。远端和本地同时修改时，必须保留 Git 的显式分叉/冲突语义。
- 如果另一台设备先 push，导致本地 push 被 rejected 或 non-fast-forward，SkillBox 会保留本地 commit，返回 `push_failed` 状态，并让 GUI 显示 push failure / retry。
- 用户需要在应用外用标准 Git 工具解决 divergent history，例如 `git fetch` 后 `git pull --rebase`、`git merge` 或其他团队约定流程；解决时应检查相关 `SKILL.md`，并在 Git 产生 conflict markers 时按普通 Git 冲突流程处理。
- 当前 GUI 不提供内置 merge editor；解决 Git 历史后，再回到 SkillBox 重试 sync。

失败与回滚：

- Git 命令失败时返回结构化错误，不吞掉 stderr。
- 没有 commit message 时使用默认 `Sync user skills`。
- 没有 configured remote 且要求 push 时拒绝同步。
- 选择文件为空且存在 changed files 时拒绝提交。
- push 失败不应修改本地提交历史；本地 commit 保留，返回 `push_failed` 状态。
- 不应把 remote URL、commit message 或 selected paths 拼成 shell 字符串。

完成验证：

- `cargo test -p skillbox-git --offline`
- `cargo test -p skillbox-core --offline user_skills`
- `cargo run -p skillbox-cli --offline -- user-skills-status --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- sync-user-skills --managed-root <temp-skillbox-root> --remote <bare-repo-path> --message "test sync"`
- UI 路径变更时，手动验证 commit review dialog、diff preview、默认 commit message、文件选择、shared remote 提示和 push failure 状态。

### Reviewed Inbound Fast-Forward

v0.7 Draft contract:

- Rust CLI:
  - `skillbox user-skills-inbound-check`
  - `skillbox user-skills-inbound-preview`
  - `skillbox user-skills-inbound-apply --preview-id <id>`
- Tauri:
  - `check_user_skills_inbound`
  - `preview_user_skills_inbound`
  - `apply_user_skills_inbound`
- Desktop Settings 的 `User skills Git` 区域使用 `Check remote`、
  `Review incoming changes`、`Apply fast-forward` 三个明确术语。

Check remote:

- 只由用户或 CLI 显式触发；v0.7 不在 startup/background 自动 fetch。
- 固定 fetch configured `origin/main`。remote 未配置、`origin/main` 不存在、
  auth/network failure 或 timeout 必须返回 actionable structured state/error。
- fetch 只更新 remote-tracking refs，不修改 working tree、index 或 SQLite skill
  rows。
- 分开返回 worktree `clean/dirty` 和 history relation：
  `unknown/synced/ahead/behind/diverged/remote_only/no_remote_branch`。
- 同时返回 local/remote/merge-base SHA、ahead/behind counts、fetch timestamp/error、
  sanitized remote URL 和 branch；不得在 UI/log 中暴露 credentials。

Review incoming changes:

- 再次显式 fetch/recompute，并提供 repository-wide review；不提供 selective
  per-skill apply。
- 将 remote changes 分组为 added/updated/deleted/renamed skills 与 repository
  files，提供有界 file diff。
- remote tree 是不可信输入。Preview 必须验证 `SKILL.md`/skill name、entry/file/tree
  大小、路径和文件类型，拒绝 `.git` content、traversal、unsafe files 和 escaping
  symlinks。
- Preview 列出受影响的 deployed runtime/profile targets。已部署 skill 的 update
  可以继续 review；已部署 skill 的 delete/rename 必须 blocked，并提示先
  undeploy。未部署 skill 的 delete 可以 apply。
- `preview_id` 绑定 local HEAD、remote SHA、merge base、remote URL、branch、
  worktree state、validated tree、diff/change grouping 和 deployment impact。
- `dirty + behind` 仍可展示 incoming review，但 Apply 必须 disabled。
- `diverged` 只显示 local-only/remote-only commit counts、双方修改的 files/skills
  和 likely conflicts。允许 Open repository、Copy repository path、Refresh；
  不提供 Keep local / Accept remote 或内置 merge editor。

Apply fast-forward:

- 必须提供当前 `preview_id`；apply 会重新 fetch、重新计算 relation/tree/deployment
  state，任何 local HEAD、remote SHA/URL/branch、dirty state 或 target state 变化都
  以 stale preview 拒绝。
- 只允许 clean `behind`，或 local unborn 且没有 user content 的安全
  `remote_only` bootstrap。生成的默认 `.gitignore` 可以被识别为非 user content；
  其它未提交文件/skill 都阻止 bootstrap。
- `ahead`、`synced`、`diverged`、`unknown`、`no_remote_branch` 不得通过该 API
  修改 working tree。
- Apply 在旧 HEAD 创建
  `refs/skillbox/backups/inbound/<operation-id>`。Rust core 从已验证 commit 的 Git
  blobs 直接物化受审文件，更新 index，并用 compare-and-swap 将 `main` 从预期旧
  SHA 推进到受审 remote SHA。入站流程不运行仓库 hook、filter、textconv、external
  diff 或 merge driver。禁止 auto merge、rebase、reset、force-push、stash、
  last-write-wins 和 conflict-marker editing。
- Apply 持有 `.git/index.lock`，使并发 `git add` / `git commit` fail closed；tracked
  文件在替换或删除前以 fd-relative no-replace rename 移入 `.git/skillbox/` 内的
  operation-scoped recovery snapshot。snapshot parent chain 通过 no-follow directory
  handle 逐级打开；receipt 绑定 backup entry 的 device/inode/size/content hash，
  backup 只以 `NOFOLLOW|NONBLOCK` 打开受限大小的真实 regular file，并拒绝 FIFO、
  special file、oversize 或读取期间增长的 entry；restore/cleanup 先将 pathname 原子
  移入私有 quarantine 再核验身份。预置或并发换入的 symlink、非目录或 replacement
  entry 不能重定向恢复写入，也不能被误当成已恢复。
  写入、ref 推进前后都会复核受审内容；检测到外部编辑时恢复或保留双方内容并要求人工
  处理，不静默覆盖。
- Incoming add/rename/type-change 如果与本地 ignored 或 untracked path 发生 exact、
  ancestor 或 descendant 碰撞，Apply 必须在 mutation 前 blocked。普通
  `git status` 未显示 ignored 内容不等于可覆盖。
- Inbound apply、outbound user-skills Git、deploy/undeploy 与其它 managed user-skill
  写操作共用 mutation lock；Save remote 与 inbound action 在 UI 也互斥。Apply 失败
  后旧 preview authorization 立即失效，必须 Refresh 并重新 review。
- 通用 managed-layout/read 初始化不补写 Git ignore defaults；显式 Git 配置/同步在
  持 mutation lock 的路径内完成该设置，避免与 remote-only bootstrap 竞争。
- Git 成功后，Rust core transactionally reconcile SQLite 中全部 user-skill index
  rows，使其对应新的 repository snapshot。若 reindex 失败，必须补偿恢复旧 HEAD，
  保留 backup ref 并返回失败；补偿只在 HEAD 仍等于 expected applied SHA 且操作写集
  未被并发改变时执行，不能 reset 掉外部 commit。Remote-only 补偿还必须清空 index，
  并以 no-replace 方式恢复 generated `.gitignore`；若其它进程已创建不同内容则保留
  外部内容并报告 partial recovery，不能截断它。Remote-only apply 会先原子转移当前
  `.gitignore` 再核验它仍是 SkillBox 生成的真实 regular file，因此 preflight 后的
  editor atomic-save replacement 或 symlink 不会被 unlink。
- Compensation 独立尝试所有仍可安全证明的 ref、index、worktree、generated defaults
  和 lock cleanup；单项失败不会跳过其它恢复。若 Git/worktree/SQLite 已成功，但
  `.git/index.lock` 的 pathname 已被外部替换，apply 返回 succeeded result 加
  actionable warning，operation phase 记录 `completed_with_warnings`，不会伪装成普通
  failed apply 或删除 replacement lock；Settings 的当前 Git workflow 会持续显示该
  warning，直到用户 dismiss。index restore 使用 `.git` dirfd 下不可预测、
  create-new/no-follow 的 private regular file；index-lock release 通过 atomic
  exchange 保持 pathname 全程占位，ownership mismatch 时原子换回外部 lock。
- Operation log 记录 aggregate old/new refs、backup ref、mutation phase 与
  compensation outcome，不记录 credentials、diff contents 或 skill bodies。
  Remote identity 必须去除 URL userinfo、query 和 fragment。
- Network fetch/push 拒绝 repository-local 与 worktree-scope credential helper、
  include/proxy、SSH/upload/receive-pack command、URL rewrite 和 protocol override；
  `GIT_ALLOW_PROTOCOL` 只允许 `file/http/https/ssh/git`，任意 `git-remote-*` custom
  helper 与 `ext` transport 均 fail closed。所有 origin/config/fetch preflight 共用
  bounded deadline 和 isolated process-group termination；按原顺序恢复用户
  global generic/URL-scoped credential helpers（包括 GitHub CLI blank reset）与
  `core.sshCommand`。`extensions.worktreeConfig` 通过 bounded Git boolean parser
  识别 `true/yes/on/1` 与 implicit true；非法值 fail closed。repo-local executable
  config 不得在 reviewed flow 中执行；global
  Git config 是用户受信任边界。helper/server stderr 只映射为有界分类错误，不直接进入
  UI、日志或 operation payload。

分叉处理：

- `diverged` 没有 Apply success path。只通过 commit/tree diff 生成 bounded
  both-changed files/skills 与 likely-conflict 诊断，不执行 remote `.gitattributes`
  driver。Rename provenance 会保留到诊断中，clean delete/delete 不标为 conflict，
  rename/delete 与 rename/rename 由 merge-tree 标出。无 merge base 的 unrelated
  histories 返回结构化 `analysis unavailable` 原因，而不是伪装成 0 conflicts。
  选择的 merge driver。用户需在 SkillBox 外使用正常 Git 工具
  fetch/merge/rebase 并检查冲突，完成后回到 SkillBox 点 Refresh/Check remote。
- 该手动解决过程不改变 outbound `sync-user-skills` 的 `push_failed` 语义：
  outbound push rejected 时仍保留 local commit。

完成验证：

- `cargo test -p skillbox-git --offline`
- `cargo test -p skillbox-core --offline inbound`
- 两个 local clones + bare remote 验证 behind preview、backup ref、fast-forward 与
  SQLite reindex。
- 验证 dirty/stale/diverged/remote-only/invalid tree/deployed delete-or-rename
  blockers 不修改 working tree 或 managed/index state。
- `cargo run -p skillbox-cli --offline -- user-skills-inbound-check --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- user-skills-inbound-preview --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- user-skills-inbound-apply --preview-id <id> --managed-root <temp-skillbox-root>`
- Desktop 手动验证 behind review、dirty+behind、diverged diagnostics 与窄 viewport。

## 12. Add Agent Adapter

触发条件：

- 需要支持 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等新的 agent runtime。
- 某个 agent 的原生格式不是 `SKILL.md` 目录，或部署路径不同于当前 `.codex/.agents` roots。

步骤：

- 先判断需求是同格式 runtime profile，还是 native format adapter。只新增
  `SKILL.md` root/capability 时扩展 Rust profile registry；非 `SKILL.md` format 才新增
  adapter。
- 定义独立的 profile/adapter id、display name、支持 scope 和默认发现路径；不要复用
  usage/call `agent_id` 作为 target identity。
- 定义原生格式读取方式：单文件、目录、规则文件、提示词文件或能力包。
- 定义如何转换为 SkillBox 规范化记录，包括 name、description、content hash、source path 和格式类型。
- 定义部署方式：symlink、copy snapshot、生成文件、或 adapter-specific materialization。
- 定义冲突规则：何时拒绝覆盖、何时备份、何时允许更新同一 SkillBox 管理目标。
- 在 Rust core 中注册 adapter，不让 React UI 直接处理 agent-specific 文件系统逻辑。
- 更新 `docs/data-model.md` 中的 schema/migration 描述。

失败与回滚：

- adapter 无法识别原生格式时，应返回候选错误而不是写入 managed store。
- 部署到 agent runtime 前必须检查目标是否存在及是否由 SkillBox 管理。
- 生成型部署失败时必须清理部分写入，或保留明确的 backup。
- adapter 不能修改其它 agent 的 runtime 目录。

完成验证：

- 新增 adapter-specific Rust tests 覆盖 scan、import、deploy、conflict 和 rollback/cleanup。
- `cargo test --offline`
- 如果 adapter 影响桌面或仓库脚本，也运行 `npm test`。
- 用临时目录模拟该 agent runtime，不直接修改真实用户 runtime。

## 13. Manage Workspaces

触发条件：

- 桌面 UI 打开 Workspaces 页面。
- 桌面 UI 或 Rust CLI 执行 workspace scan。
- 用户通过手动路径或打包版 Tauri app 的原生单目录选择器添加现有 skills root，或选择普通项目目录并显式初始化一个受支持的项目局部 root。
- 用户按 workspace 名称、路径或 agent 搜索，并可与 Global/User 类型组合过滤。
- 用户点击 workspace 查看其中 skills，并选择导入。
- Dashboard scan import candidates 时自动登记已扫描的 workspace。

步骤：

- `workspace-scan` 调用 Rust profile registry，按明确 precedence 发现存在且可读取的
  `.agents/skills`、`.codex/skills`、`.claude/skills`、`.cursor/skills` roots。
- home-level roots 记录为 `kind=global`；项目局部 roots 记录为 `kind=user`。
- Rust 写入 `profile_id/root_key/format`；legacy `agent_id` 只为兼容保留，React 不根据
  path marker 推断 runtime identity。
- display name 由 path 推导：global root 使用 agent 名，项目局部 root 使用项目目录名，不拼接 `global` 或 `user`。
- 扫描每个 workspace root，记录 skill 数、已导入 skill 数、scan error 数和最后一条 scan error。
- 点击 workspace 时只扫描该 workspace path，复用 import candidate review 行样式展示其中的 skills，并使用现有 `import_candidates` 流程导入选中项。
- Add workspace 的 `Project` 选项继续持久化为兼容现有 registry 的 `kind=user`；`Global` 继续持久化为 `kind=global`，不需要 schema migration。
- `Project or skills folder` 保留可编辑的绝对路径输入；打包版 Tauri app 额外提供原生目录选择器，只允许选择一个本地目录，不接受文件或多选。浏览器 prototype 不把 file input 当成可信绝对路径来源。
- 原生选择器返回目录后，UI 将绝对路径写入现有输入框，清除旧 preview、root selection 和 error，并在保持当前 Project/Global scope 的前提下立即调用只读 workspace setup preview；用户取消选择时保留当前路径和 preview，不显示错误。
- workspace setup preview 是只读操作。现有 skills-root path 可直接登记；普通项目目录
  只检测 registry v1 的 `.agents/skills`、`.codex/skills`、
  `.claude/skills`、`.cursor/skills`。
- 若项目中存在一个或多个受支持 root，用户必须选择其中一个登记。若不存在，UI 只允许选择并创建一个 root；已有 runtime marker 优先推荐对应 root，否则确定性默认 `.agents/skills`。
- apply 会重新校验 preview identity、项目 canonical boundary、固定 relative-root allowlist、目录类型和 symlink 边界，然后才创建并登记。不会修改现有项目文件，也不会同时创建多个 runtime roots。
- Global setup 只允许登记已存在目录，不推断或创建 global root。旧 `workspace-add <path> --kind user|global` CLI contract 继续只登记已存在目录。
- 忘记 workspace 只允许删除 `source=manual` 的 registry row，不删除或修改磁盘文件。
- Workspace 搜索只过滤当前已登记的 rows，不触发文件系统扫描；清空 query 后恢复当前类型下的全部 rows。

失败与回滚：

- 不存在的手动 path 拒绝添加。
- 目录选择器取消是 no-op；picker/plugin 错误以内联可操作错误显示，不替换现有路径，也不绕过 setup preview。
- setup preview 不写 managed store、SQLite 或项目目录；缺失 root 只在用户确认 `Create & add` 后创建。
- traversal、project/candidate symlink、逃逸项目 canonical boundary、现有非目录 target、不可读目录、stale/tampered preview 都拒绝 apply；用户明确选择的精确 skills-root symlink 仍按旧 contract 解析到其真实目录后登记。
- apply 后注册失败时，只反向移除本次操作新建且仍为空的目录；不删除预先存在的目录或内容。
- 自动 scan 跳过不存在或不可读取的 roots。
- scan error 记录在 workspace 行上，不中断其它 workspace。
- forget 不能删除 auto workspace，也不能删除 runtime 目录中的内容。

完成验证：

- `cargo test -p skillbox-core --offline workspace`
- `cargo run -p skillbox-cli --offline -- workspace-scan --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- workspace-add <temp-root> --kind user --managed-root <temp-skillbox-root>`
- `npm test`
- 桌面 UI 验证 sidebar 只保留 Dashboard、Workspaces、Settings；Workspace 页面可 search、组合类型筛选、scan、add、forget manual rows，并覆盖 manual path、native single-directory picker、picker cancellation/error、existing-root、no-root initialization、multiple-root selection 和窄窗口状态。

## 14. Skill Usage Evidence Recording

触发条件：

- agent adapter、CLI wrapper、runtime hook 或 history sync 产生可分类的本机 skill
  usage evidence。
- Rust CLI 入口：`usage-record`。
- Rust CLI hook 入口：`usage-hook codex|claude-code`、`usage-hook-status`、`usage-hook-install <target>`。
- Tauri command：`record_skill_usage`。
- Tauri hook 配置 command：`usage_hook_statuses`、`install_usage_hook`。
- 桌面 Settings 的 `Usage hook injection` 一键注入 Codex App、Codex CLI 和 Claude Code CLI 的 Stop hook。
- 不统计 SkillBox 打开详情、部署、更新、scan、import 或其它管理行为。

步骤：

- 调用方提交 `skill_name`、`agent_id`、`runtime_root`，可选提交 `event_id`、`used_at` 和 `metadata`。
- hook 注入只修改对应 agent 的配置文件；Codex App 和 Codex CLI 共享 `~/.codex/hooks.json`，Claude Code CLI 使用 `~/.claude/settings.json`。
- Codex App / Codex CLI 的非 managed command hook 写入后仍需用户在 Codex `/hooks` 中 review/trust；Settings 显示 `Needs trust` 时表示文件已注入但自动统计尚不会执行。
- 注入命令必须指向 `~/.skillbox/bin/skillbox-usage-hook <agent>`；SkillBox 安装或重新注入时写入同目录 `skillbox-usage-hook-runner`，并替换旧的裸 `skillbox usage-hook ...` 或开发态绝对路径配置，避免命中 legacy Node CLI、找不到命令，或依赖 `target/debug`。
- 注入命令挂在 `Stop` 事件上。hook 命令读取 agent 提供的 `transcript_path`，只提取本 turn 中 Skill 块的 `name`、`path` 和触发用户 prompt 的受限 excerpt；不保存完整 prompt、聊天正文、文件内容或 transcript。
- `usage-hook` 命令必须 fail-open：解析或写入失败时不应让 agent hook 返回失败，从而不影响 agent 会话结束。
- Rust core 写入 `skill_usage_events`，允许 `skill_name` 尚未导入 SkillBox；每条 row
  持有当前最强 `confirmed/inferred/reference` class 和有界 provenance sources。
- `Calls = confirmed + inferred`。`reference` 单独作为 History references，不进入 Calls。
- 普通 `usage-record` 默认是 `reference`，并只在同一
  `agent_id + runtime_root + event_id` 已存在时返回 deduplicated。由 core 内部生成的
  trusted hook/backfill 使用稳定 event id；当 deployment attribution 改变时，core 可在
  canonical agent aliases 范围内按 `skill_name + event_id` 复用首次 runtime root。
  公开请求即使伪造 reserved source 也不能启用这条跨 runtime 去重或提高 evidence。
- 相同 canonical event 收到更强证据时执行
  `reference -> inferred -> confirmed` 单向升级，不创建第二条 event；弱证据不能降级，
  所有已观测 source 保留在 `evidence_sources_json`。
- 写入或升级成功后，`skill_usage_stats` 按
  `skill_name + agent_id + runtime_root` 从 Calls evidence 聚合；reference 不进入 stats。
- `used_at` 不传时使用当前 UTC RFC3339 时间；`recorded_at` 始终记录 SkillBox 收到上报的时间。
- `metadata` 必须是 JSON object，大小受限，不能包含 prompt、聊天正文、文件内容、diff
  等内容型字段。公开 `usage-record` 不能设置
  `agent_hook|codex_session_backfill|claude_code_session_backfill|cursor_session_backfill|cursor_agent_transcript_read`
  等保留 source。
- 桌面 skill 详情页按 `skill_name` 汇总 Calls 与次级 History references；Workspace card
  按 runtime root 显示 Calls，workspace skill/import 行使用同一 Calls 口径。
- 桌面 skill card 在 skill name 下方直接显示全局 Calls。

失败与回滚：

- 无效 skill name、agent id、runtime root、timestamp 或 metadata 时拒绝写入。
- hook 注入前如果配置文件存在，先写同目录 `.bak` 备份；无效 JSON 配置拒绝注入，不覆盖原文件。
- usage event 不写 operation log；operation log 只记录 SkillBox 管理动作。
- usage 写入失败不应影响 scan/import/deploy 工作流。
- schema v7 migration 不扫描 agent history；它在 transaction 中回填 evidence 并从
  `confirmed + inferred` 幂等重建 stats。升级后无需 rescan，用户显式运行
  `Sync histories` 时才恢复新 evidence 或升级旧 event。

完成验证：

- `cargo test -p skillbox-core --offline usage`
- `cargo test -p skillbox-core --offline usage_hook`
- `cargo test -p skillbox-cli --offline usage_record`
- `cargo test -p skillbox-cli --offline usage_hook`
- `cargo run -p skillbox-cli --offline -- usage-audit --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- usage-record --skill <skill-name> --agent <agent-id> --runtime-root <runtime-root> --event-id <id> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- usage-hook-install codex-app`
- `cargo run -p skillbox-cli --offline -- usage-hook-status`
- 使用相同 `--event-id` 重复上报，确认第二次返回 deduplicated 且计数不增加。
- `npm test`

### 14.1 Evidence-aware Skill Usage Rankings

触发条件：

- Rust core 入口：`list_skill_usage_rankings`、`usage_audit`、
  `backfill_codex_session_usage`、`backfill_claude_code_session_usage`、
  `backfill_cursor_session_usage`。
- Rust CLI 入口：`usage-rankings [--range 7d|30d|all] [--type user|remote|system] [--agent <id>] [--workspace <runtime-root>] [--include-unmanaged]`、
  `usage-audit`、`usage-backfill-codex [--include-archived]`、
  `usage-backfill-claude-code [--projects-root <path>]`、
  `usage-backfill-cursor [--database-path <path>]`。
- Tauri command：`list_skill_usage_rankings`、`usage_audit` 和三个 provider backfill。
- 桌面左侧一级导航进入 Rankings 页面，或修改时间、skill type（User/Remote/System）、Agent、Workspace 过滤器；也可点击 `Sync histories`。

步骤：

- 默认查询最近 30 天，用户可切换最近 7 天或全部历史；时间窗按 `used_at` 计算，并排除晚于本次查询时间的未来事件。
- skill type 过滤只接受 User、Remote、System：User/Remote 按当前 managed store 中的类型分类，System 按事件的可信 source identity 分类；`Not imported`、`Deleted`、`Unknown source` 是管理或来源状态，不属于 skill type。筛选在 coverage 累计前执行，因此 rows、rank、total 和 coverage 使用同一快照口径。
- Agent 使用与 usage event 相同的 normalized `agent_id`（写入时把路径型 `agents`/`claude` 规范为 `codex`/`claude-code`；过滤同时兼容历史遗留 id）；Workspace 表示事件写入时的 canonical runtime root。实时 hook 优先使用结构化 `runtime_root` 或 `cwd` 在同一 managed skill 的多个 deployment 中定位 workspace，session 回填使用 session metadata 的 `cwd`，无上下文时才使用确定性 fallback。
- 默认结果包含当前 managed store 中的 User/Remote skills（无 Calls evidence 时为
  `0 calls`），以及时间窗内有 Calls 或 reference evidence、但尚未导入 SkillBox 的
  unmanaged skills；CLI 可用 `--include-unmanaged` 显式打开同一范围，桌面 Rankings
  默认开启。reference-only skill 的 Calls 必须为 0，只能显示次级 History references。
  现有 `Not imported`、`Deleted`、`System`、`Unknown source` 与 Import 限制保持不变，
  且不因其它 Workspace 同名目录串号。
- 聚合来源是 `skill_usage_events`。Calls 只累计当前最强 class 为
  `confirmed|inferred` 的 event；History references 只累计 `reference`。同一 skill
  跨 Agent/Workspace 的同类 evidence 在未过滤时相加。
- 排序固定为 Calls 降序、last used time 降序、skill name 升序、source identity 升序，
  随后分配连续 ordinal rank；reference 数量不能提升默认排名。
- 桌面以可访问的数据表展示精确名次、skill 名称、Calls、last used time、次级
  History references 和 Actions；History 把 Call、History reference 和管理操作作为
  不同 kind/filter 展示。
- Rankings 页提供 `Sync histories`，顺序运行三个互相独立的本地 provider 并在完成后刷新一次 ranking；单个 provider 失败时继续运行其余 provider，notice 明确成功/失败来源，已成功写入的幂等事件不回滚：
  - Codex 流式扫描本机 `~/.codex/sessions`（可选 `archived_sessions`）中的
    `rollout-*.jsonl`（不跟随 symlink），只从 user turn 的完整
    `<skill><name>/<path>` block 或 `[$skill](.../SKILL.md)` link 提取绝对、可解析的
    `SKILL.md`。这是逐回合 `inferred` invocation，计入 Calls，但不是
    provider-native execution。catalog、普通 prose、相对路径、代码模板、
    `exec_command`、custom/dynamic tool payload、assistant/tool/shell output 均排除。
  - Claude Code 扫描 `~/.claude/projects/**/*.jsonl`（不跟随 symlink），只接受
    assistant record 中原生 Skill tool use 或 Skill command attribution，并解析到可读
    `SKILL.md`；这类 evidence 是 `confirmed`，自由文本 mention 不记录。
  - Cursor 以 read-only、`query_only` 和 bounded busy timeout 打开本机
    `state.vscdb` 并验证 private schema。non-subagent human bubble 中
    `addedWithoutMention=false context.cursorRules[].filename` 只证明上下文附加，记录为
    `reference`。此外有界扫描 Cursor agent transcripts；assistant `tool_use` 的
    `Read` 输入为绝对本机 `SKILL.md` 路径时，按 transcript user turn + skill 记录
    `cursor_agent_transcript_read=inferred`。现存文件必须通过 traversal、symlink、
    allowed-root、regular-file、大小和 frontmatter 检查；后来已移动/删除的文件只在
    lexical path 与最近现存 ancestor 都未逃逸 allowed root、parent skill name 合法时
    保留 historical evidence，并且该 path 不得用于 filesystem/deploy authority。
    `ReadFile` candidates 只进入 aggregate diagnostics，不计 Calls。
  - provider 使用稳定 provider/session/turn/path identity。Cursor 同一 user turn
    对同一 skill 的重复 Read 只计一次；缺少 preceding user record 时使用每个 transcript
    唯一的 `unattributed` fallback turn，因此仍然保守去重。重复 sync 不递增；新强证据
    可以升级旧 event 并保留 provenance。非法或不支持记录计入 skipped/errors。
- 桌面 Full ranking 表格包含 Actions 列：已导入 skill 显示 `Detail` 并打开既有 skill 详情弹窗；未导入 skill 显示 `Import`，通过 row 的 `source_id + source_kind + source_runtime_roots`，以及生成该行的 ranking filters 和 `generated_at` 调用 source-aware `preview_usage_skill_import`。core 必须用同一查询快照重建完整 row identity，拒绝缺失 identity、任意 roots 子集、被篡改或已过期的请求；只在重建出的全部 roots 中选择 Importable candidate，某个 root 已失效时可继续检查同一 source 的其他 root，但不得回退到任意全局 runtime 或删除备份。旧的 name-only Rust API 保留原有本地恢复搜索，但 Tauri Rankings command 始终要求完整 identity。候选确认后用户可选择导入为 User 或 Remote，确认后写入 managed store 并刷新 Rankings。同名普通 skill 与 System/Unknown source 即使共享 runtime root 或普通副本已 managed 也拆成独立行；System 与 Unknown source 均不可 Import。
- Rankings 页面宽度与 Dashboard 一致，不再单独收窄。
- 紧凑 UI 使用 `Calls` / `<n> calls`。详情/help 必须解释 Calls 是本机
  `confirmed + defensible inferred`，History references 是显式 mention/context，
  两者都不是 Codex/Claude account analytics。`0 calls` 只表示 SkillBox 当前没有 Calls
  evidence，不表示从未使用；usage frequency 不改变 source trust、安全性或质量判断。
- 每个 ranking 查询返回同一过滤快照的 `total_calls`、confirmed/inferred/reference
  totals 和各自最早/最新时间。evidence-class totals 按 event 当前最强 class 互斥，且
  `confirmed + inferred = total_calls`；reference 单列。
- coverage `source_counts` 从 `evidence_sources_json` 统计 provenance。一个被 hook
  confirmed 的 Codex inferred event 会保留两个 source，因此 source counts 可以重叠，
  不要求其总和等于 Calls 或 event total。provider 最近一次扫描的文件/session/turn 和
  backfill discovered/recorded/deduplicated/upgraded/skipped/errors 是独立操作覆盖。
- Codex 本地 stores 没有稳定 provider-native skill-run total。当前 Codex Calls 是
  confirmed hook 加结构化 per-turn inferred invocation 的已知下界，仍可能 undercount；
  SkillBox 不从 prose、catalog、shell/tool payload 或 output 补数。
- `usage-audit` 只返回上述 aggregate counts、时间覆盖、scan/backfill totals 和已知限制，
  不返回 prompt、chat body、tool payload/output、credentials 或完整 metadata。
- 未来若接入 Codex reported runs，必须按 provider、subject kind、time window、scope 和 provenance 独立存储与展示；不得写入 `skill_usage_events`，不得参与本地 ranking、total 或 delta。
- 查询只读取 `metadata_json.skill_source_kind` 这一受限身份字段，响应不返回 `prompt_excerpt` 或完整 `metadata_json`；usage 数据不上传、不跨设备合并，也不作为社区排行榜。
- 导入预览或 backfill 进行中切换离开 Rankings 时，迟到响应不得再打开确认框或把错误写到其它页面；loading 标记仍须清理，避免返回后按钮永久禁用。

失败与回滚：

- 非法 range、agent id 或相对 Workspace path 拒绝查询；读取失败只展示错误，不修改 usage events、managed store 或 runtime。
- history sync 只读各 agent 的本机会话存储；解析或单条写入失败计入 skipped/errors，不中断同一 provider 的其余可读记录，也不修改 agent 会话、数据库或 hook 配置。Cursor 私有 schema 不满足白名单时整个 Cursor provider fail closed。
- schema v4 增加 ranking indexes；schema v5 规范 legacy agent ids 并清理重复 event；
  schema v7 保守回填 evidence/provenance、增加 evidence indexes，并从
  `confirmed + inferred` events 幂等重建 stats。已有数据库升级前按通用规则备份，失败
  transaction 回滚；migration 不扫描 agent history，不要求 rescan，显式
  `Sync histories` 才会恢复或升级 evidence。
- 快速连续切换过滤器时，桌面只应用最后一个请求的结果，避免旧响应覆盖新条件。

完成验证：

- `cargo test -p skillbox-core --offline usage_ranking`
- `cargo test -p skillbox-core --offline usage_backfill`
- `cargo test -p skillbox-core --offline cursor_backfill`
- `cargo test -p skillbox-core --offline usage_evidence`
- `cargo test -p skillbox-core --offline usage_audit`
- `cargo test -p skillbox-core --offline schema_v`
- `cargo test -p skillbox-cli --offline usage_ranking`
- `cargo run -p skillbox-cli --offline -- usage-rankings --range 30d --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- usage-backfill-codex --sessions-root <temp-sessions> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- usage-backfill-claude-code --projects-root <temp-claude-projects> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- usage-backfill-cursor --database-path <temp-state-vscdb> --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- usage-audit --managed-root <temp-skillbox-root>`
- `node --test apps/desktop/src/usageRankings.test.js apps/desktop/src/cardLayout.test.js`
- `npm test`
- 桌面手动验证 Rankings 的 7/30/all、User/Remote/System、Agent/Workspace 组合过滤、Sync histories（含单 provider 失败）、空状态、键盘 focus、最后请求获胜和 managed skill detail 跳转。

## 15. App Updates

触发条件：

- 桌面 app 启动后请求一次后台检查；Rust 对同一 current version 的成功结果执行 24 小时节流。
- app 连续运行时每小时触发一次轻量 due-check；未到 24 小时时只返回 SQLite 中的最近成功快照，不访问网络。
- 用户在 Settings -> App updates 点击 `Check for updates`。
- 发现更新后，用户点击侧边栏品牌旁的 `Update`，或在 Settings 点击 `Install and restart`。

步骤：

- React 自动调用 `check_app_update(force=false)`，手动检查调用 `force=true`；React 不直接下载 release asset，也不解析任意 URL。
- Tauri command 通过 HTTPS 使用 Tauri updater plugin 读取和解析 `latest.json`；metadata check 不等同于 artifact 验签。
- debug/dev/browser preview 不访问 GitHub，返回 disabled 状态。
- 成功检查的展示快照写入 managed SQLite `preferences.app_update_check_cache`。同一 current version 在 24 小时内复用该快照；损坏、未来时间、过期或版本不匹配的快照都不会阻止新检查。
- SQLite cache 写入失败时记录 desktop stderr，并保留进程内成功快照继续执行 24 小时节流；跨启动恢复会在持久化恢复后重新生效。
- 有可用更新时，Tauri 保存最近一次成功 metadata check 得到的进程内 pending update；侧边栏显示 Update，Settings 显示版本、notes 和安装按钮。
- 点击安装时先执行一次 force check，确认版本仍然可用，再调用 `install_app_update` 下载、验证、安装并重启。进程内 pending 缺失时，Rust 也必须重新检查，不能从 SQLite cache 构造 URL 或安装对象。
- 自动检查只检查 metadata；没有用户点击时不下载、不安装、不重启。
- Release workflow 必须上传 DMG、updater `.app.tar.gz`、`.sig` 和 `latest.json`；`latest.json` 同时包含 `darwin-aarch64` 和 `darwin-x86_64`，指向同一个 universal updater archive。

失败与回滚：

- force recheck 后没有可用版本时清除提醒并报告已是最新；Rust fallback 仍在没有 pending/update 时拒绝安装。
- updater check/download/install 失败时展示错误，不修改 managed store 或 runtime skills。
- 自动网络检查失败不覆盖最近一次成功 cache，也不弹出阻塞提示；当前进程后续 due-check 可以重试。
- 下载或安装失败时保留 pending update，允许用户直接重试。
- 用户点击后，Tauri updater plugin 下载 updater asset，并在安装前验证 artifact 签名；签名失败时拒绝安装。
- 丢失 `TAURI_SIGNING_PRIVATE_KEY` 会导致已安装用户无法接受未来更新，必须保留离线备份。

完成验证：

- `cargo test -p skillbox-core --offline app_update_check_cache`
- `cargo test -p skillbox-desktop --offline app_update`
- `npm test`
- `npm --workspace apps/desktop run build`
- Release workflow `workflow_dispatch` dry run 必须验证 DMG、updater archive、signature 和 `latest.json`。
- 正式发布后，用前一版 DMG 安装包验证：首次检查出现侧边栏提醒；重启后 24 小时内恢复提醒且不重复联网；点击一次即可重新检查、安装并重启。

## 16. Database Migrations And Doctor

触发条件：

- `ensure_managed_layout` 打开或创建 `skillbox.sqlite`。
- Rust CLI 执行 `doctor [--repair-preview]`。
- 桌面 Settings -> Health 执行 `Run health check`。

步骤：

- 数据库通过 `schema_migrations` 记录已经应用的 migration version 和名称。
- 每个待处理 migration 在独立 transaction 中按 version 顺序执行。
- 已有非空数据库在首次执行待处理 migration 前通过 SQLite 一致性快照生成一次 backup；新数据库不生成 backup。
- migration decision、backup 和 migration application 由 per-database process-safe lock 串行化；多个 desktop/Tauri/CLI caller 并发初始化时只能生成一份 backup，并按一次顺序迁移完成。
- migration 完成后运行 SQLite integrity check。
- Doctor 只读检查 schema/integrity、user/remote managed skill 结构、remote `current` symlink、deployment、workspace、active import backup/source 和 stale skill metadata。
- Doctor 比较 deployment symlink 时允许 `~/.skillbox` 与其 legacy `~/SkillBox` 目录别名，但 remote deployment 仍必须指向 `current` 入口，不能直接固定到某个 version。
- managed skill 和 runtime target 都不存在时，将 deployment row 报告为可清理的 warning；如果 runtime target 仍存在，则保留 error 并要求人工检查，不能自动删除目标。
- `repair_preview=true` 只返回建议动作，不修改文件系统或数据库记录。
- 用户显式执行 `doctor-clean-stale-deployments` 或桌面 `Clean stale records` 时，只删除 managed skill 与 runtime target 都不存在的 SQLite deployment rows；不删除任何 runtime 文件，并记录 `repair_stale_deployments` operation。

失败与回滚：

- migration transaction 失败时不写入对应 `schema_migrations` row。
- migration 失败时保留升级前 backup，下一次启动可重试未完成 version。
- Doctor 无法安全判断修复方式时返回 `repairable=false`，不能猜测或覆盖目标。
- 清理前会再次确认 managed skill 和 runtime target 仍不存在；任一目标存在时保留记录。

完成验证：

- `cargo test -p skillbox-core --offline database`
- `cargo test -p skillbox-core --offline doctor`
- `cargo run -p skillbox-cli --offline -- doctor --repair-preview --managed-root <temp-skillbox-root>`
- `cargo run -p skillbox-cli --offline -- doctor-clean-stale-deployments --managed-root <temp-skillbox-root>`
- `npm test`

## 17. Persist Skill User Metadata

触发条件：

- 用户在 Dashboard 或 Skill Detail 切换 favorite 或编辑 tags。
- 桌面首次升级后仍存在 legacy dashboard metadata local-storage keys。

步骤：

- Rust core 校验 skill name，规范化、去重并限制 tags，然后 upsert `skill_user_metadata`。
- 桌面启动读取 SQLite metadata，供 Dashboard filters 和 Skill Detail 使用。
- legacy metadata 通过批量 `INSERT OR IGNORE` 迁移，不能覆盖 SQLite 中已经存在的记录。
- legacy migration 成功后删除旧 local-storage keys；之后 SQLite 是唯一真相源。

完成验证：

- `cargo test -p skillbox-core --offline skill_user_metadata`
- `node --test apps/desktop/src/skillUserMetadata.test.js`
- `npm --workspace apps/desktop run build`

## 18. CLI And Desktop Capability Matrix

Both interfaces call the same Rust core for filesystem, Git, GitHub, SQLite,
migration, compatibility, and recovery behavior. “Both” below means the
workflow exists in both interfaces; presentation can differ because the CLI
returns structured JSON while the desktop provides interactive review.

| Workflow | Rust CLI | Desktop | Notes |
| --- | --- | --- | --- |
| Initialize/read the managed store | Full | Full | CLI exposes `init` and `paths`; desktop initializes on startup and shows managed roots in Settings. |
| Scan common or explicit roots | Full | Full | CLI `scan` accepts explicit roots for automation; desktop scans common roots and registered workspaces. |
| Review and import existing skills | Partial | Full | CLI `import` accepts one validated source directory. Desktop groups scan candidates, shows duplicate/source context, and supports reviewed deploy-back. |
| Preview-confirmed GitHub install | Full | Full | Both require an install preview ID; warning targets require explicit confirmation. |
| Runtime profiles and deployment compatibility | Full | Full | CLI exposes `runtime-profiles`, `deploy-preview`, and `deploy`; desktop shows profile metadata and an interactive compatibility review. |
| Undeploy and reviewed skill deletion | Full | Full | Both share overwrite, ownership, stale-preview, and confirmation protections. |
| Remote source binding, update, rollback, and versions | Full | Full | Desktop provides all-file visual review; CLI returns structured diff/version data. |
| User-skills Git status and outbound commit/push | Full | Full | Desktop adds selected-file diff review. Existing push defaults and `push_failed` semantics remain unchanged. |
| Reviewed inbound user-skills fast-forward (Unreleased v0.7 Draft) | Full | Full | Both use Check -> Preview -> Apply with the same Rust validation and stale-preview contract. Desktop adds visual repository/skill/deployment review and conflict diagnostics. Neither interface auto-merges, rebases, resets, stashes, or resolves divergence. |
| Workspaces | Partial | Full | CLI lists/scans/adds/forgets exact roots. Desktop also previews a project directory, initializes one selected supported root, and offers the native folder picker. |
| Usage rankings and local history sync | Full | Full | Both use the same confirmed/inferred/reference evidence model and provider backfills. |
| Aggregate usage diagnostics | Full | Limited | CLI `usage-audit` is the automation-oriented aggregate report. Desktop exposes the relevant coverage summary and disclosure, not the complete diagnostic JSON. |
| Usage hook status/install | Full | Full | CLI supports Codex and Claude Code hook automation; desktop exposes the supported hook controls in Settings. |
| History and operation audit | Partial | Full | CLI exposes operation rows plus separate usage/ranking surfaces; desktop combines Calls, References, and operations in one History timeline. |
| Doctor and safe stale-record cleanup | Full | Full | Both share the read-only health report and bounded cleanup behavior. |
| Import record inspection and revert | Full | Full | Desktop places interactive review in Skill Detail; CLI uses `import-records` and `revert-import`. |
| App update check/install | Not available | Full | Signed updater state and install/restart are packaged-app interactions. CLI releases are installed through the distribution channel. |
| UI preferences, favorites, and tags | Not available | Full | These are desktop interaction preferences persisted through Rust/Tauri. |

Intentionally unsupported in both interfaces:

- automatic Git pull/merge/rebase or an in-app merge editor;
- silent overwrite of non-symlink runtime content;
- deployment without a fresh compatibility preview;
- native non-`SKILL.md` Claude, OpenClaw, Cursor, Claude Code, or Copilot
  format adapters;
- provider-account analytics or fabricated usage totals.

Maintenance check:

```sh
cargo run -p skillbox-cli --offline -- --help
rg -n "#\\[tauri::command\\]|invoke_handler|invoke\\(" \
  apps/desktop/src-tauri/src apps/desktop/src/App.jsx
```

Update this matrix whenever a CLI command, Tauri command, or desktop workflow
changes its user-visible availability.
