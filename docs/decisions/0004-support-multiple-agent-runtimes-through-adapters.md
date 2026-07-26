# ADR 0004: 通过 Adapter 支持多 Agent Runtime

## 背景

SkillBox 面向多 agent runtime。目标用户会同时使用 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等 agent，而这些工具的目录布局、原生格式、部署语义和冲突风险不同。

## 决定

SkillBox 的 managed store 保持 agent-agnostic。每个 agent runtime 通过 adapter 接入：

- adapter 负责发现 runtime roots。
- adapter 负责读取该 agent 的原生格式。
- adapter 负责把原生内容映射到 SkillBox 的规范化记录。
- adapter 负责部署回该 agent 的 runtime，并声明 symlink、copy snapshot 或生成文件等模式。

在完整 native adapter 之前，`SKILL.md` 兼容 runtime 使用一层 Rust-owned runtime profile：

- registry 有明确版本，v1 内建 `agents`、`codex`、`claude-code`、`cursor` 和
  `custom-skill-md`，不由用户 JSON 动态修改。
- profile 声明受支持 roots、确定性 precedence、format、frontmatter policy 和
  deployment modes。
- `workspace.profile_id/root_key/format` 表达 runtime target identity；usage
  telemetry 的 `agent_id` 保持独立，不作为部署身份。
- compatibility preview 是只读操作，保留未知 optional frontmatter 并报告 warning；
  malformed metadata、required incompatibility、unsafe path、foreign target 或
  unsupported mode 会 blocked。
- apply 必须重新计算 preview identity；skill snapshot、target state/canonical path、
  profile metadata 或 registry version 变化时拒绝 stale preview。

## 理由

- 单一 `SKILL.md` 目录模型无法表达所有主流 agent 的能力格式。
- UI 不应该知道 Claude、Cursor、Copilot 等工具的目录细节。
- adapter 边界能让扫描、导入、部署和冲突处理继续在 Rust core 中测试。
- runtime profile 让当前同格式 roots 先共享一套可测试 contract，而不是在 React
  中按路径猜 agent。
- managed store 作为真相源，可以避免不同 agent runtime 互相覆盖。

## 后果

- 当前 `.agents/.codex/.claude/.cursor` profile 都只表示 `SKILL.md` roots，
  不代表对应产品的完整 native format 已受支持。
- 新增 agent 支持时，必须同时更新 architecture、data model、workflow 和测试。
- adapter 可以选择非 symlink 部署，但必须提供与 symlink 默认模式同级别的冲突保护和回滚路径。
- SQLite schema v6 在现有 workspace registry 上增加 `profile_id`、`root_key` 和
  `format`，并保留 legacy `agent_id` 供兼容读取；不得把它与 usage `agent_id` 混用。
