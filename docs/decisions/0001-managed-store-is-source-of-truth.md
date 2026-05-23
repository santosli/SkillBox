# ADR 0001: Managed Store 是真相源

## 背景

Skills、规则、提示词和能力包可能存在于 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等不同 agent 的全局 runtime 目录、项目局部 runtime 目录、用户手动维护目录和远程下载目录中。如果 SkillBox 直接以某个 runtime 目录为准，扫描、导入、更新、回滚和同步都会互相踩状态。

## 决定

`~/SkillBox` 是 SkillBox 管理状态的真相源：

- `~/SkillBox/user-skills` 保存用户创建的 skills。
- `~/SkillBox/remote-skills` 保存远程或手动远程导入的版本历史。
- Runtime 目录只作为部署目标。

## 理由

- 用户创建内容需要一个稳定位置，才能做 Git 同步、备份和编辑。
- 远程 skill 需要保存版本历史，才能 update 和 rollback。
- Runtime 目录可能由 Codex、Claude、OpenClaw、Cursor、Claude Code、Copilot、项目局部配置或用户手动修改，不能表达完整来源和版本语义。
- 以 managed store 为中心可以让 UI、CLI 和测试共享同一套状态模型。

## 后果

- 扫描 runtime 目录只能发现候选项或部署状态，不能直接把 runtime 当作最终状态。
- 导入 runtime 目录中的既有 skill 时，必须复制到 managed store。
- 迁移回 runtime 时默认创建 symlink，而不是把 managed store 的控制权交还给 runtime 目录。
- 删除或覆盖用户内容前必须有明确确认和备份路径。
