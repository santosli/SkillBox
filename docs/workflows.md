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

- 读取当前已实现的默认 runtime roots：`~/.codex/skills`、`~/.agents/skills`，以及发现到的项目局部 `.codex/skills`、`.agents/skills`。
- 后续通过 agent adapter 读取 Claude、OpenClaw、Cursor、Claude Code、Copilot 等 runtime roots。
- 在每个 root 内递归查找包含 `SKILL.md` 的目录。
- 读取 frontmatter 中的 `name`、`description`、`version`。
- 计算 `SKILL.md` content hash。
- 标记 source root、是否 symlink、real path。
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

- Node CLI 当前入口：`skillbox check-updates [skill-name]`。
- 目标入口：Rust core + Rust CLI + Tauri command。

步骤：

- 遍历 `remote-skills/<name>/source.json`。
- 只处理 `type: github` 的 remote skill。
- 使用 `git ls-remote <repoUrl> <ref>` 查询最新 SHA。
- 比较 `latestSha` 与 `installedSha`。
- 返回 `skillName`、`installedSha`、`latestSha`、`updateAvailable`。

失败与回滚：

- 缺失 `source.json` 的 remote skill 跳过或标记不可检查。
- 非 GitHub remote 跳过。
- 网络或 Git 失败应作为该 skill 的 update check error 返回，不应破坏现有版本。

完成验证：

- 当前 Node：`node packages/skillbox-cli/bin/skillbox.js check-updates --managed-root <temp-SkillBox> --json`
- Rust 迁移完成后新增 tests 覆盖 missing source、manual source、GitHub source 和 Git failure。

## 6. Update Remote Skill

触发条件：

- 目标 workflow，当前 Rust/UI 未完整实现。
- Node core 当前有 update check，但没有完整的 update command。

步骤：

- 先执行 check updates。
- 如果没有新 SHA，返回 no-op。
- 下载或 checkout 最新 commit。
- 验证 `SKILL.md` 和 skill name。
- 写入 `versions/<latestSha>`。
- 更新 `current` symlink。
- 更新 `source.json.installedSha` 和 `latestSha`。
- 记录 SQLite skill hash/path 状态。
- 保留旧版本目录，供 rollback 使用。

失败与回滚：

- 下载失败不改变 `current`。
- 新版本无效时拒绝更新，并保留旧版本。
- 更新 `current` symlink 失败时恢复到旧 `current`。
- 不删除旧版本目录。

完成验证：

- 新增 Rust tests 覆盖 no-op update、新版本写入、旧版本保留和 symlink 恢复。
- 手动验证：安装一个固定旧 ref 后更新到新 ref，确认 `current` 指向新 SHA。

## 7. Rollback Remote Skill

触发条件：

- Node CLI 当前入口：`skillbox rollback <skill-name> --to <sha>`。
- 目标入口：Rust core + Rust CLI + Tauri command。

步骤：

- 校验 skill name。
- 在 `remote-skills/<name>/versions` 查找等于 rollback 参数或以该参数开头的版本目录。
- 验证目标版本包含 `SKILL.md`。
- 更新 `current` symlink 指向目标版本。
- 更新必要的 SQLite 状态。

失败与回滚：

- 找不到版本时拒绝。
- 短 SHA 匹配多个版本时应拒绝。
- symlink 更新失败时恢复到原 `current`。
- 不删除任何 version 目录。

完成验证：

- 当前 Node：`node packages/skillbox-cli/bin/skillbox.js rollback <skill-name> --to <sha> --managed-root <temp-SkillBox> --json`
- Rust 迁移完成后新增 tests 覆盖完整 SHA、短 SHA、无匹配、多匹配和 symlink restore。

## 8. Sync User-Skills Git

触发条件：

- Node CLI 当前入口：`skillbox sync-user-skills [--remote <git-url>] [--message <msg>] [--push]`。
- 目标入口：Rust core + Rust CLI + Tauri command。

步骤：

- 确保 `~/SkillBox/user-skills` 存在。
- 如果没有 `.git`，初始化 Git 仓库。
- 如果提供 remote，设置或更新 `origin`。
- `git add .`。
- 如果有变更且提供 commit message，创建 commit。
- 如果 `--push`，推送到 `origin main`。
- 返回 initialized、branch、dirty、raw status、committed、pushed。

失败与回滚：

- Git 命令失败时返回结构化错误，不吞掉 stderr。
- 没有 commit message 时不自动提交。
- push 失败不应修改本地提交历史。
- 不应把 remote URL 或 commit message 拼成 shell 字符串。

完成验证：

- 当前 Node：`node packages/skillbox-cli/bin/skillbox.js sync-user-skills --managed-root <temp-SkillBox> --message "test sync" --json`
- Git status：Rust `skillbox-git` tests 或手动调用覆盖 dirty/clean 仓库。
- Rust 迁移完成后新增 tests 覆盖 init、remote add/set-url、no-op、commit 和 push failure。

## 9. Add Agent Adapter

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
