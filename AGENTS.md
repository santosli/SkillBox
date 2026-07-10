# SkillBox Agent Guide

## 项目目标

SkillBox 是一个本地 macOS 应用和 CLI，用来管理主流 agent 可用的 skills、规则、提示词和能力包。
覆盖 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等 agent 生态。

SkillBox 管理两类内容：

- 用户创建的 skills，存放在 `~/.skillbox/user-skills`。
- 远程下载或导入的 skills，存放在 `~/.skillbox/remote-skills`。

`~/.skillbox` 是 SkillBox 的真相源。各 agent 的 runtime 目录只应被当作部署目标，
例如 `~/.codex/skills`、`~/.agents/skills`、项目局部 runtime，或后续 adapter 支持的 Claude、Cursor、Copilot 等目录。

## 必守规则

- 业务逻辑优先放在 Rust crates 中，桌面 UI 通过 Tauri commands 调用核心能力。
- React 层不能直接拥有文件系统、Git、GitHub、下载、迁移或回滚行为。
- 新增核心业务逻辑不要引入 legacy Node CLI；CLI 和桌面入口共享 Rust core。
- 文件系统操作必须显式、可验证，并尽量具备备份或回滚路径。
- 不要执行用户提供的 shell 字符串；使用结构化参数和校验后的路径。
- GitHub URL、远程归档、外部路径和现有 runtime skills 都是不可信输入。
- 不要静默覆盖 runtime 目录中的既有 skill，尤其不能覆盖非 symlink 目标。
- 除非用户明确确认破坏性操作，否则必须保留用户创建的 skill 内容。
- 不要把某个 agent 的格式当成全局格式；跨 agent 行为必须通过 adapter 或明确的兼容层表达。

## 当前实现边界

- Rust 已覆盖 `SKILL.md` 目录的扫描、导入、候选导入、GitHub install、symlink 部署、SQLite 基础索引、GitHub URL 解析、Git 状态读取和 user-skills Git 同步。
- Tauri 桌面桥接当前调用 Rust commands，CLI 入口也调用 Rust core；legacy Node CLI/core 已退役。
- Claude、OpenClaw、Cursor、Claude Code、Copilot 等非 `SKILL.md` 或非 Codex-style runtime 的支持尚需 agent adapter 层。
- Node/npm 仅保留为桌面前端、仓库脚本和测试运行时，不承载 SkillBox 产品业务逻辑。

## 文档导航

- 版本演进路径和 1.0 晋级门槛：`docs/roadmap.md`
- 系统地图和模块边界：`docs/architecture.md`
- 存储布局、SQLite、命名和兼容规则：`docs/data-model.md`
- 可执行 workflow 和完成标准：`docs/workflows.md`
- 本地开发、测试和提交规范：`CONTRIBUTING.md`
- 架构决策记录：`docs/decisions/`
- 当前实现进度快照：`docs/implementation-status.md`

## 验证要求

每个有意义的改动都必须包含自动化测试，或给出清楚的手动验证记录。

在声称某个 workflow 完成之前，必须运行对应测试或命令，并报告验证内容。
需要保持可验证的 workflow 见 `docs/workflows.md`。

## 分支与子 Agent 协作要求

复杂任务在修改文件前，主 Agent 必须先检查当前分支和工作区状态，并创建或切换到
`codex/<short-slug>` 任务分支；不得直接在 `main` 上开发或累积复杂任务改动。已经位于与
当前任务匹配的非 `main` 分支时可以继续使用，不必重复建分支。

满足以下任一条件即视为复杂任务：

- 属于下文“文档同步要求”定义的重大变更；
- 同时涉及两个或以上产品层或模块边界，例如 Rust core、CLI、Tauri bridge、React UI、
  SQLite、Git/GitHub 或 release tooling；
- 涉及 schema/migration、数据恢复、安全或信任边界、破坏性文件系统行为、runtime
  adapter、签名、发布或分发；
- 包含两个或以上可独立验证的实现子任务，或预计需要多个 focused commits。

开始复杂任务时：

1. 先运行 `git status --short --branch`，确认当前分支和已有改动；
2. 工作区干净时，从最新可用的 `main` 创建 `codex/<short-slug>`；
3. 如果 `main` 上已有当前任务的未提交改动，不得丢弃或自动 stash，先创建任务分支保留改动；
4. 如果已有改动属于其它任务且无法安全隔离，停止修改并向用户说明冲突。

只读分析、诊断、代码审查以及不满足上述条件的单文件小修正不强制创建分支。仓库规定必须从
干净 `main` 运行的正式 release automation 是正常例外，但待发布的功能实现必须已经通过
任务分支完成并集成；其它例外必须由用户明确授权。

复杂任务制定计划时，必须主动识别可独立并行的子任务。当存在边界清楚、不会修改同一文件或
共享可变状态的子任务，并且有可用 Agent 容量时，应至少优先委派一个子任务给子 Agent。
适合委派的任务包括只读调研、独立模块实现、独立测试、文档核对和最终审查。

主 Agent 负责分支准备、任务拆分、接口约定、结果集成、冲突处理、完整测试和最终汇报；主对话
只保留关键决策、进度与综合结论，不重复展开每个子任务的内部过程。子 Agent 的任务必须有明确
范围、允许修改的文件和验收标准。由于 Agent 共享同一工作区：

- 不得让多个 Agent 同时修改相同文件；
- 不得让子 Agent 并发执行 branch switch、stage、commit、merge、release、全仓格式化或
  会批量重写文件的生成命令；
- Git 状态变更、跨模块集成和最终全量验证统一由主 Agent 执行；
- 子 Agent 完成后，主 Agent 必须检查实际 diff 和测试证据，不能直接把其结论当作完成。

任务过小、步骤存在严格前后依赖、无法划分非重叠文件、涉及破坏性或唯一外部状态、Agent 容量
不可用，或委派成本明显高于任务本身时，可以不使用子 Agent；主 Agent 应在进度说明中简短记录原因。

## 文档同步要求

重大变更必须在同一个 change set 中同步更新对应文档，不能等到发版时补写，也不能只更新代码。
以下任一情况都视为重大变更：

- 新增、删除或改变用户可见 workflow、CLI/Tauri contract 或受支持 runtime；
- 改变存储布局、SQLite schema、migration、备份、恢复或兼容策略；
- 改变核心模块边界、adapter 设计、source-of-truth、安全边界或破坏性操作保护；
- 改变版本里程碑的范围、顺序、完成状态或晋级门槛；
- 改变安装、升级、签名、发布、回滚或分发方式。

按变更内容至少同步以下文档：

- 里程碑范围或状态：`docs/roadmap.md` 和 `docs/implementation-status.md`；
- 架构或信任边界：`docs/architecture.md`，必要时新增或更新 `docs/decisions/*`；
- 数据和迁移：`docs/data-model.md`；
- 用户 workflow 和完成标准：`docs/workflows.md`；
- 用户可见能力或发布内容：`README.md`、`README.zh-CN.md` 和 `CHANGELOG.md`；
- 开发、测试或发布规范：`CONTRIBUTING.md` 和相关 release 文档。

重大变更未完成对应文档和验证时，不得标记 workflow 完成、不得声称里程碑完成、不得发版。
`SKILLBOX_SKIP_DOCS_CHECK=1` 只允许用于确认不影响上述内容的内部小改动，不能用于绕过重大变更的文档同步。

## 提交规范

所有提交必须使用 Conventional Commits，并写英文提交信息。

格式：

```text
<type>(<scope>): <summary>
```

允许的 type：`feat`、`fix`、`docs`、`test`、`refactor`、`chore`、`build`、`ci`、`perf`、`style`。

允许的 scope：`desktop`、`core`、`cli`、`scan`、`import`、`docs`、`hooks`、`github`。

summary 必须简洁具体，不要使用 `update`、`fix stuff`、`improve things` 这类模糊描述。
如果一次提交包含多个不相关变更，先拆分提交。

提交前的 `.githooks/pre-commit` 会检查 staged implementation / workflow 变更是否需要同步更新
`AGENTS.md`、`README.md`、`CONTRIBUTING.md` 或 `docs/*`。只有确认属于不影响上述文档类别的
内部小改动时，才可以用 `SKILLBOX_SKIP_DOCS_CHECK=1 git commit ...` 显式跳过。
提交前的 `.githooks/commit-msg` 会校验提交信息是否符合本节规则。
