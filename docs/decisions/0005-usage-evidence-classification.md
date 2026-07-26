# ADR 0005: 按证据强度分类本机 Skill Usage

## 背景

SkillBox 从 runtime hooks 和本机会话历史恢复 skill usage。不同来源能证明的事实不同：
Stop hook 或原生 Skill tool 可以证明本机执行，结构化的逐回合 skill attachment 可以证明
用户发起了调用，但普通文本、catalog、shell/tool payload 或历史上下文引用不能证明执行。
如果把这些来源全部计为 Calls，Dashboard、Workspace、History 和 Rankings 会把“提到过”
误报为“调用过”；如果全部排除历史信号，又会系统性低估没有 hook 覆盖的 Codex 使用。

Codex 当前本地 store 没有稳定、专用且可用于恢复 provider-native skill-run total 的事件。
因此 SkillBox 只能给出有证据边界的本机计数，不能声称与 Codex 账户或产品 analytics 相等。

## 决定

SQLite schema v7 为每个 `skill_usage_events` row 增加一个当前最强
`evidence_class`，并用有界的 `evidence_sources_json` 保留已观测 provenance：

- `confirmed`：结构化证据能证明本机执行或加载。
- `inferred`：结构化、逐回合信号能可靠证明调用意图，但没有 provider-native execution
  结果。
- `reference`：只证明 skill 被提及、附加为上下文或由未受信调用方上报。

用户可见的 `Calls` 定义为：

```text
Calls = confirmed + defensible inferred
```

`reference` 单独显示为 History references，不进入 Calls、默认排名或
`skill_usage_stats`。

初始来源分类：

- `agent_hook` 是 `confirmed`。Stop hook 从已完成 turn 的 transcript 中读取受限的
  structured skill blocks。
- Claude Code 原生 Skill tool use 或原生 Skill command attribution 是
  `confirmed`。自由文本中的 Claude/Skill mention 不是证据。
- Codex user turn 中完整 `<skill><name>/<path>` block 或
  `[$skill](.../SKILL.md)` link 是 `inferred`。它们是逐回合、绝对路径校验后的
  invocation carrier，不是普通 catalog/prose，但也不是 provider-native execution
  result。
- Cursor `context.cursorRules` state 只证明 skill 被附加为上下文，是 `reference`。
- Cursor agent transcript 中 assistant `tool_use` 的 `Read` 是 execution proxy，
  不是 provider-confirmed execution，因此是 `inferred`。调用单位是每个稳定 transcript
  user turn、每个 skill 一次；同 turn 重复 Read 去重。现存文件执行严格
  traversal/symlink/regular-file/size/frontmatter 校验。后来移动/删除的路径只在
  absolute/local、精确 `SKILL.md` suffix、合法 parent skill name、lexical allowed-root
  和最近现存 ancestor containment 均成立时保留 historical evidence；它永远不是
  filesystem/deploy authority。`ReadFile` 的语义尚未完成 qualification，只报告
  aggregate candidates，不计 Calls。
- 公开 `usage-record` 没有 trusted parser 证据，默认是 `reference`。
- catalog、普通 user/assistant prose、`SKILL.md` mention、`exec_command`、任意
  custom/dynamic tool payload、tool output 和 shell output 均不构成 Calls。

同一 canonical provider/session/turn/skill event 只保留一条 row。新证据强于旧证据时，
`reference -> inferred -> confirmed` 单向升级；弱证据不能降级。升级不增加 event 数，
并保留所有已观测来源。provenance source count 不是互斥 event partition，不能要求其总和
等于 Calls 或总事件数。

schema v7 migration 在 transaction 中保守回填 evidence、重建
`skill_usage_stats`，且可重复执行。迁移不扫描 agent history，也不要求用户 rescan；
显式 `Sync histories` 可以重放稳定事件身份，恢复新来源或把旧 evidence 升级。

`usage-audit` 只返回 aggregate：各 evidence/source 的数量和时间覆盖、扫描文件/session
数量，以及最近 backfill 的 discovered/recorded/deduplicated/upgraded/skipped/errors。
它不得返回 prompt、chat body、tool payload/output、credentials 或完整 metadata。

## 理由

- Calls 需要同时避免把 reference 当执行，也避免把结构化 Codex invocation 全部丢弃。
- 单一最强 class 让排序、过滤和 all-time stats 有稳定口径；provenance list 保留审计证据。
- 升级而不是追加 event，能让 history sync 与实时 hook 幂等汇合。
- read-only migration 和显式 history sync 避免升级时静默扫描私有会话。
- aggregate-only audit 可以解释覆盖缺口，而不扩大本地聊天数据暴露面。

## 后果

- Dashboard、Workspace、History Calls、Rankings 和默认排序只读取
  `confirmed + inferred`。
- History references 是独立次级指标；reference-only skill 不显示为已调用。
- coverage 同时返回 evidence-class totals 与 provenance source counts。前者按当前最强
  class 互斥，后者可能重叠。
- Codex Calls 是已确认 hook 加结构化 inferred invocation 的本机下界，仍可能 undercount；
  SkillBox 不提供、推导或补齐 Codex provider-native total。
- 将来接入 provider-reported runs 时，必须继续使用独立存储和展示，不能混入本地 Calls。
