# SkillBox 产品宣传片 · v0.9.5

75 秒产品总览，展示 SkillBox 的核心：**散在各处的技能，收进一个库**。

| 文件 | 说明 |
| --- | --- |
| `skillbox-product-promo.mp4` | 成片，1920×1080、30 fps、75.000 秒、H.264 + AAC |
| `skillbox-product-promo-poster.jpg` | 封面，取自 24.0s（Skill detail） |

## 分镜

| 时间 | 内容 | 上镜组件 |
| --- | --- | --- |
| 0–7.5s | 散在各处的技能，收进一个库 | — |
| 7.5–18.75s | 所有技能，一屏管完 | `Dashboard` |
| 18.75–30s | 一处管理，多处生效 | `SkillDetailDialog` |
| 30–41.25s | 不把任何一个 runtime 当成唯一真相源 | `WorkspacePage` |
| 41.25–52.5s | 写入之前，先看清楚 | `ImportReview` |
| 52.5–65.625s | 用了多少，都说得出依据 | `UsageRankingsPage`（含 coverage 展开） |
| 65.625–75s | 本地优先，先审阅再写入 | — |

## 与 v0.9.0 宣传片的区别

`../skillbox-intro/` 那支是 30 秒、深色技术风、用产品截图做主视觉。
这一支不用截图：功能镜头在浏览器里**挂载 `apps/desktop/src` 的真实 React 组件**，并加载产品自己的
`colors.css` + `styles.css`，因此画面就是用户真正会看到的界面。Usage 的 coverage 展开态是组件
自己的 `useState`，由影片在挂载后对真实的 `.usageCoverageToggle` 按钮发一次 click 得到，
而不是复刻一份展开后的静态标记。

## 可复现工程

源工程不在本仓库，位于 `~/zone/skillbox-product-video/`，用
[guizang-product-video-skill](https://github.com/op7418/guizang-product-video-skill) 制作。
那里保留了分镜（`plan.json`）、组件接线（`src/presentations.jsx`）、代码原创配乐（`tools/score.py`）、
卖点证据与验收记录（`evidence/`）。本目录只归档成片与封面。

## 演示数据

影片里的使用数据、部署目标和集合来源是**虚构的演示数据**，不含真实账号、密钥、路径或商业结果；
每处自备数据及其原因记录在源工程的 `evidence/feature-evidence.md`。

## 声音

配乐为代码原创（128 BPM、40 小节、固定随机种子，Python 标准库 + FFmpeg 合成），
音效来自制作 skill 的内置原创 WAV。成片 −16.09 LUFS / −1.29 dBTP。
