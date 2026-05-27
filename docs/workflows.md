# SkillBox Workflows

本文件定义工作流入口、步骤、失败处理和完成标准。实现位置和长期目标见 `docs/architecture.md`。
SkillBox 的目标是跨 agent 管理，不只覆盖 Codex。当前 workflow 以 `SKILL.md` / Codex-style roots 为第一阶段实现；
Claude、OpenClaw、Cursor、Claude Code、Copilot 等需要通过 agent adapter 扩展。

## 1. Scan Local Skill Roots

触发条件：

- UI 刷新 managed state 或扫描 import candidates。
- Rust CLI 执行 `scan`。
- Node CLI 执行 `scan`，用于兼容和回归对照。

步骤：

- 读取当前已实现的默认 runtime roots：`~/.codex/skills`、`~/.agents/skills`、`~/.claude/skills`，以及发现到的项目局部 `.codex/skills`、`.agents/skills`、`.claude/skills`。
- 后续通过 agent adapter 读取 Claude、OpenClaw、Cursor、Claude Code、Copilot 等 runtime roots。
- 在每个 root 内递归查找包含 `SKILL.md` 的目录。
- 读取 frontmatter 中的 `name`、`description`、`version`。
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
- `node packages/skillbox-cli/bin/skillbox.js scan --json`

## 2. Import Existing Skills

触发条件：

- UI first-use 或用户主动扫描本机已有 skills。
- Rust CLI 执行 `import`。
- Node CLI 执行 `import`，用于兼容入口。

步骤：

- 扫描 import candidates。
- 根据路径和内容推断类型：当前 `.agents/skills` 倾向 user，`.codex/skills` 倾向 remote，`.system` 默认不选中，包含 GitHub 来源信息的未知目录倾向 remote。
- agent adapter 引入后，候选项还应携带 `agent_id`、原生格式和 target scope。
- 检查 managed target 是否冲突。
- user skill 复制到 `~/SkillBox/user-skills/<name>`。
- remote skill 复制到 `~/SkillBox/remote-skills/<name>/versions/manual-<contentHash12>`，并更新 `current` symlink。
- 如果用户选择 deploy back to source，先把原 runtime 目录移动到 `~/SkillBox/backups/imports/<name>-<contentHash12>`，再在原位置创建指向 managed target 的 symlink。
- 写入 SQLite `skills`，必要时写入 `deployments`。

失败与回滚：

- managed target 已存在且 hash 不一致时拒绝。
- 原 runtime 位置是指向其它位置的 symlink 时拒绝。
- deploy back to source 创建 symlink 失败时，应把 backup rename 回原位置。
- 不覆盖用户内容，不删除 backup。

完成验证：

- `cargo test --offline`
- `npm test`
- 使用临时目录运行 Rust CLI：`cargo run -p skillbox-cli --offline -- import <source-dir> --type user --managed-root <temp-SkillBox>`
- UI 路径变更时，手动验证 import review 中冲突、默认选中和备份提示。

## 3. GitHub Install

触发条件：

- Node CLI 当前入口：`skillbox install <github-url> [--target <path>]`。
- UI 当前只支持 URL parse 和提示，尚未执行下载导入。
- 目标入口：Rust core + Rust CLI + Tauri command。

步骤：

- 解析 GitHub tree、blob、raw 或 contents API URL。
- 标准化 owner、repo、ref、path、repoUrl、url。
- 用结构化 Git 参数执行 clone、sparse checkout 或等价下载流程。
- 验证下载目录包含 `SKILL.md`。
- 读取 skill name 并校验命名。
- 写入 `remote-skills/<name>/versions/<installedSha>`。
- 更新 `remote-skills/<name>/current` symlink。
- 写入 `source.json`，包含 GitHub 来源和 `installedSha`、`latestSha`。
- 写入 SQLite `skills`。
- 如果提供 target，执行 deploy workflow。

失败与回滚：

- URL 不指向 skill 目录或 `SKILL.md` 时拒绝。
- Git 命令失败时清理临时目录，不写 managed store。
- version 已存在时可以复用，但仍需验证 `SKILL.md`。
- target 部署失败时保留已安装版本，并把 deployment error 返回给调用方。

完成验证：

- 当前 Node：`node packages/skillbox-cli/bin/skillbox.js install <github-url> --managed-root <temp-SkillBox> --json`
- URL parse：`cargo run -p skillbox-cli --offline -- parse-github-url <github-url>`
- Rust 迁移完成后必须新增 Rust tests 覆盖 URL parse、版本目录、`source.json` 和 target deploy。

## 4. Deploy Managed Skill

触发条件：

- Rust CLI 执行 `deploy <skill-name> --target <path>`。
- Node CLI 执行 `deploy`。
- import workflow 选择 deploy back to source。

步骤：

- 校验 skill name。
- 在 managed store 中解析 user skill 或 remote `current`。
- 创建 target root。
- target 不存在时创建 symlink。
- target 是 symlink 且已指向同一 managed path 时视为成功。
- 写入 SQLite `deployments`。

失败与回滚：

- target 是非 symlink 时拒绝。
- target 是 symlink 但指向其它位置时拒绝。
- 创建 symlink 失败时不写 deployment 记录。
- 不删除非 SkillBox 管理的内容。

完成验证：

- `cargo test --offline`
- `npm test`
- `cargo run -p skillbox-cli --offline -- deploy <skill-name> --target <temp-runtime> --managed-root <temp-SkillBox>`
- 检查 target path 是 symlink，real path 指向 managed store。

## 5. Check Remote Updates

触发条件：

- Rust CLI 当前入口：`cargo run -p skillbox-cli --offline -- check-remote-updates [--managed-root <temp-SkillBox>]`。
- Tauri command：`check_remote_skill_updates`。
- 桌面启动只调用 `cached_remote_skill_updates` 读取上一次检查结果，不主动查询远端。
- Node CLI 兼容入口仍有 `skillbox check-updates [skill-name]`，但桌面 UI 不调用 Node。

步骤：

- 遍历 `remote-skills/<name>/source.json`。
- 只处理 `type: github` 的 remote skill。
- `refKind: tag`、`refKind: commit` 或 `tracking: false` 的 GitHub source 标记为 `pinned`，不执行远端更新判断。
- 对 tracking branch 使用 `git ls-remote <repoUrl> <ref>` 查询最新 SHA。
- `git ls-remote` 必须以非交互方式执行，并设置有界超时；超时或网络错误只标记对应 skill 为 `Check failed`，不能拖住整个 Refresh。
- 优先比较 latest remote SHA 与 `currentVersion`；没有 `currentVersion` 时兼容比较 `installedSha`。
- 返回每个 remote skill 的 `skillName`、`sourceType`、`currentVersion`、`installedSha`、`latestSha`、`refKind`、`tracking`、`updateAvailable`、`state`、`message`。
- 成功执行远端检查后，把完整检查结果和检查时间缓存到 managed SQLite preferences；下次桌面启动复用缓存状态，只有用户刷新或自动刷新后才更新缓存。
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
- `cargo run -p skillbox-cli --offline -- check-remote-updates --managed-root <temp-SkillBox>`
- `npm test`
- 桌面 UI 视觉验证 Dashboard `Refresh` 按钮、Checked 时间、状态 badge、Available updates 计数、notice，以及 Settings 中的自动刷新间隔。

## 6. Bind Remote Source

触发条件：

- Rust CLI 当前入口：`remote-source-candidates`、`remote-source-preview`、`bind-remote-source`。
- Tauri command：`find_remote_source_candidates`、`preview_remote_source_binding`、`bind_remote_source`。
- 桌面 `Bind source` 弹窗打开时会后台调用 `find_remote_source_candidates`，候选只用于预览，仍需用户确认后才绑定。
- 用户为已有 remote skill 手动添加 GitHub source URL。
- 用户触发 Claude Marketplace candidate search，为已有 remote skill 自动寻找可能的 source。
- MVP 只接受 GitHub skill directory 或 `SKILL.md` URL。

步骤：

- 自动搜索调用 `https://claudemarketplaces.com/api/skills` 拉取 Claude Marketplace skills 列表，本地按 skill name 精确命中优先过滤；没有精确命中时再退到 name/path contains。
- 桌面自动搜索必须先渲染弹窗和后台搜索提示；搜索期间用户仍可手动粘贴 URL 或关闭弹窗。
- 自动搜索把 marketplace 结果映射回 GitHub source URL，结果按 skill name、path、marketplace install signal 和 stars 排序。
- 自动搜索只返回候选、score 和 match reasons，不写 `source.json`，不修改版本目录，必须由用户确认后继续绑定。
- 绑定前校验会先尝试候选 URL 的原始 path；若 marketplace path 是逻辑 skill 名称而不是仓库真实目录，继续尝试 `skills/<name>`、`skills/public/<name>`、`.claude/skills/<name>` 等常见布局，并把成功解析出的 GitHub URL 写入预览和 `source.json`。
- 校验本地 skill name，并解析 GitHub URL 的 owner、repo、ref 和 path。
- 在临时工作树中 fetch 目标 ref，并只 checkout URL 指向的 skill path。
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
- `cargo run -p skillbox-cli --offline -- remote-source-candidates <skill-name> --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- remote-source-preview <skill-name> <github-url> --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- bind-remote-source <skill-name> <github-url> --managed-root <temp-SkillBox>`
- 桌面 UI 手动验证 source binding dialog：`exact_match` 可绑定，`same_skill_changed` 明确提示当前版本不会被替换，`mismatch` 禁用绑定。

## 7. Update Remote Skill

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
- 在临时工作树中 fetch 目标 ref，并只 checkout `source.json.path` 对应的 skill 目录。
- 验证 `SKILL.md` 和 skill name。
- 应用前对当前 `current` 目录和目标 snapshot 生成 no-index diff；diff 必须包含所有新增、修改、删除文件，路径规范化为 skill 内相对路径。
- diff preview 对二进制文件或超过 120 KB 的文件保留文件行、hash 和 size，但不展开文本 diff。
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
- `cargo run -p skillbox-cli --offline -- remote-versions <skill-name> --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- remote-preview-change <skill-name> --action update --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- remote-apply-change <skill-name> --action update --to <sha> --managed-root <temp-SkillBox>`
- 手动验证：安装一个固定旧 ref 后更新到新 ref，确认 `current` 指向新 SHA。
- 桌面 UI 手动验证：update review 打开期间显示 loading，完成后展示所有变更文件，文本文件展示 unified diff，二进制或大文件展示 hash/size metadata，no-file-change 更新显示明确说明，确认后刷新版本列表和 operation history。
- Tauri 验证：`preview_remote_version_change` 这类 Git/diff 预览 command 必须放到 blocking worker，避免点击 `Review update` 时阻塞窗口渲染。

## 8. Rollback Remote Skill

触发条件：

- Node CLI legacy 入口：`skillbox rollback <skill-name> --to <sha>`。
- Rust CLI 当前入口：`remote-versions`、`remote-preview-change --action rollback`、`remote-apply-change --action rollback`。
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

- 当前 Node：`node packages/skillbox-cli/bin/skillbox.js rollback <skill-name> --to <sha> --managed-root <temp-SkillBox> --json`
- `cargo test -p skillbox-core --offline remote_version`
- `cargo test -p skillbox-core --offline apply_`
- `cargo run -p skillbox-cli --offline -- remote-preview-change <skill-name> --action rollback --to <sha-or-prefix> --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- remote-apply-change <skill-name> --action rollback --to <sha-or-prefix> --managed-root <temp-SkillBox>`
- 桌面 UI 手动验证：rollback review 展示回滚后会新增、修改、删除的所有文件，确认后 `current` 和版本列表同步刷新。

## 9. Operation Log

触发条件：

- Rust core 执行会改变 managed store、runtime、SQLite、Git state 或偏好设置的动作。
- 当前 remote source bind、remote update apply、remote rollback apply 必须写 operation log；后续其它 side-effect workflow 接入同一能力。
- Rust CLI 入口：`operations`。
- Tauri command：`list_operations`。
- 桌面 UI：remote skill detail 默认折叠最近的 skill operation history，只显示日志入口和事件数；展开后每条记录显示完成时间，未完成时显示开始时间；未来 Settings 或 Operations 页面可展示全局日志。

步骤：

- 操作开始时写入 `started` record，包含 operation type、actor、entity type/name、started time、summary 和 payload。
- 操作成功时更新为 `succeeded`，写入 finished time 和最终 payload。
- 操作失败、验证拒绝或恢复失败时更新为 `failed`，写入 finished time、error 和恢复相关 payload。
- 记录由 Rust core append/update；React 只能读取展示，不能编辑、删除或伪造记录。
- MVP 永久保留 operation log，不自动清理。

失败与回滚：

- 业务操作失败时必须尽量把对应 operation 标记为 `failed`。
- operation 写入失败不能静默吞掉；调用方应收到错误或包含日志失败说明的结果。
- UI 无法加载 operation history 时，只在该 skill 的操作区展示加载失败，不阻断其它 skill 管理能力。

完成验证：

- `cargo test -p skillbox-core --offline operation`
- `cargo run -p skillbox-cli --offline -- operations --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- operations --entity-type skill --entity-name <skill-name> --managed-root <temp-SkillBox>`
- 桌面 UI 手动验证 remote skill detail 中成功和失败 operation 都可见。

## 10. Sync User-Skills Git

触发条件：

- Rust CLI 入口：`skillbox sync-user-skills [--remote <git-url>] [--message <msg>] [--no-push]`。
- Rust CLI 状态入口：`skillbox user-skills-status`。
- Tauri command：`user_skills_git_status`、`user_skills_git_changes`、`set_user_skills_git_remote` 和 `sync_user_skills_git`。
- Node CLI 仍保留 legacy 兼容入口：`skillbox sync-user-skills [--remote <git-url>] [--message <msg>] [--push]`。

步骤：

- 确保 `~/SkillBox/user-skills` 存在。
- 默认所有本地 user skills 通过同一个 `~/SkillBox/user-skills` Git 仓库和同一个 `origin` remote 同步。
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
- `cargo run -p skillbox-cli --offline -- user-skills-status --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- sync-user-skills --managed-root <temp-SkillBox> --remote <bare-repo-path> --message "test sync"`
- UI 路径变更时，手动验证 commit review dialog、diff preview、默认 commit message、文件选择、shared remote 提示和 push failure 状态。

## 11. Add Agent Adapter

触发条件：

- 需要支持 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等新的 agent runtime。
- 某个 agent 的原生格式不是 `SKILL.md` 目录，或部署路径不同于当前 `.codex/.agents` roots。

步骤：

- 定义 `agent_id`、display name、支持的 scope 和默认发现路径。
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
- 如果 adapter 影响 legacy Node CLI 兼容入口，也运行 `npm test`。
- 用临时目录模拟该 agent runtime，不直接修改真实用户 runtime。

## 12. Manage Workspaces

触发条件：

- 桌面 UI 打开 Workspaces 页面。
- 桌面 UI 或 Rust CLI 执行 workspace scan。
- 用户手动添加或忘记 workspace。
- 用户点击 workspace 查看其中 skills，并选择导入。
- Dashboard scan import candidates 时自动登记已扫描的 workspace。

步骤：

- `workspace-scan` 调用 Rust core 发现存在且可读取的 `.codex/skills`、`.agents/skills`、`.claude/skills` roots。
- home-level roots 记录为 `kind=global`；项目局部 roots 记录为 `kind=user`。
- 根据路径推断 `agent_id`：`.codex` -> `codex`，`.agents` -> `agents`，`.claude` -> `claude`。
- display name 由 path 推导：global root 使用 agent 名，项目局部 root 使用项目目录名，不拼接 `global` 或 `user`。
- 扫描每个 workspace root，记录 skill 数、已导入 skill 数、scan error 数和最后一条 scan error。
- 点击 workspace 时只扫描该 workspace path，复用 import candidate review 行样式展示其中的 skills，并使用现有 `import_candidates` 流程导入选中项。
- 手动添加 workspace 时必须提供已存在目录，并立即扫描该目录。
- 忘记 workspace 只允许删除 `source=manual` 的 registry row，不删除或修改磁盘文件。

失败与回滚：

- 不存在的手动 path 拒绝添加。
- 自动 scan 跳过不存在或不可读取的 roots。
- scan error 记录在 workspace 行上，不中断其它 workspace。
- forget 不能删除 auto workspace，也不能删除 runtime 目录中的内容。

完成验证：

- `cargo test -p skillbox-core --offline workspace`
- `cargo run -p skillbox-cli --offline -- workspace-scan --managed-root <temp-SkillBox>`
- `cargo run -p skillbox-cli --offline -- workspace-add <temp-root> --kind user --managed-root <temp-SkillBox>`
- `npm test`
- 桌面 UI 验证 sidebar 只保留 Dashboard、Workspaces、Settings，Workspace 页面可 scan、add、forget manual rows，并且点击 workspace 可查看和导入该 workspace 下的 skills。
