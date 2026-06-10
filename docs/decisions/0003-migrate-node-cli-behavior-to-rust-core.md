# ADR 0003: 将 Node CLI 行为迁移到 Rust Core

## 背景

SkillBox 早期用 dependency-free Node MVP 快速验证了扫描、导入、部署、GitHub install、update check、rollback 和 user-skills Git sync。当前桌面应用已经切到 Tauri + Rust commands，Rust crates 也覆盖了核心扫描、导入、部署和 URL 解析。

如果继续在 Node CLI 中扩展新业务，UI、Rust CLI 和 Node CLI 会分裂成多套行为。

## 决定

新增核心业务逻辑进入 Rust crates。Node CLI 保留为 legacy transition layer，直到它独有的能力迁移到 Rust：

- GitHub install
- check updates
- update remote skill
- rollback remote skill
- sync user-skills Git

## 理由

- Tauri 后端天然调用 Rust，减少 UI shell 到 CLI 的复杂度。
- Rust crates 可以被桌面 app 和 Rust CLI 复用。
- 文件系统、Git 和远程输入处理需要更强类型和更明确的错误边界。
- 单一核心实现能减少 Node/Rust/UI schema 分叉。

## 后果

- Rust 迁移旧能力时，要兼容 Node MVP 已写入的目录、`source.json` 和 SQLite 数据。
- 完成迁移前，不应移除 Node CLI 的现有测试和命令。

## 完成状态

2026-06-10：legacy Node CLI/core 已退役。`packages/skillbox-core` 和
`packages/skillbox-cli` 被移除，产品业务入口统一到 `crates/skillbox-core` 和
`crates/skillbox-cli`。Node/npm 仅保留为桌面前端、仓库脚本和测试运行时，不再承载
SkillBox 产品业务逻辑。

Rust CLI 保留常用 legacy 命令别名（例如 `install`、`check-updates`、`rollback`、
`init`、`version`），但业务实现来自 Rust core，不再通过 Node 包转发。
