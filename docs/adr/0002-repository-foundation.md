# ADR 0002：通用仓库底层与本项目适配

日期：2026-10-14（Asia/Shanghai）。状态：已接受的本地治理基线；远端执行状态见 current。

## 背景

产品 / MVP 文档已有基线，但仓库底层只有简要原则，没有模板、Ruleset 声明或可重复检查。本次按用户要求只读参考 Radish、RadishMind、RadishCatalyst、RadishFlow、RadishAxiom，提取长期共性；不复制业务实现、全量规范或当期 CI。

## 决策与取舍

- 单维护者默认 dev 串行推进，master 为稳定主线及目标默认分支。普通贡献明确 base=dev，阶段晋级经 PR 到 master，随后回流。统一明确默认分支，消除兄弟文档中“默认 / 稳定 / 开发”混用。
- 允许 merge / rebase merge，禁止 squash；晋级优先 merge。回流以 ancestry 验证，使用快进或普通 merge，禁止重写共享历史。
- master 使用严格 Candidate Quality、0 审批、讨论解决、禁止删除 / 强推；管理员 bypass 仅 PR 内，作为有记录的例外。dev 不强制 PR、不自动跑 push CI。
- 先建立真正执行文档 / 治理验证的 Repo Hygiene，未来按实际工程加入 Rust / no_std / VM 组件。CI 聚合拒绝跳过、取消和缺失结果，不预建假业务 job。
- 检查器用 Python 标准库，可在尚无 Git 时运行；workflow 采用可由 json 模块解析的 YAML 子集。暂不为治理搭建 Rust xtask 或安装 YAML 库；真实工程建立后若迁移，替换单一实现，不保留双份规则。
- 原创许可继续待定；不导入兄弟 LICENSE、资产、运行依赖、服务、公开仓库身份或安全报告地址。

## 参考与适配

以下为只读工作树参考及当时 HEAD，非上游版本锁；只记录相关文件和提炼结论，不认为所有兄弟文档都一致或正确。

| 项目 / HEAD | 阅读入口 | 采用 / 不采用 |
| --- | --- | --- |
| Radish / a99d8608 | Docs/adr/0001-branch-and-pr-governance.md、Docs/guide/agent-collaboration.md | 三级验证、日常 dev；不搬入 .NET / 前端 / 部署门禁；祖先方向按 Git 语义纠正 |
| RadishMind / 0fb3a461 | docs/adr/0001-branch-and-pr-governance.md、docs/agent-collaboration.md | 拓扑回流不重复无关验证；不迁入模型、Web、Go、数据库流程 |
| RadishCatalyst / 0bad4600 | docs/adr/0001-branch-and-pr-governance.md、docs/process/agent-collaboration.md、PR 模板 | 根入口 / 专题 / 当前状态分层；不迁入 Godot、存档、周志与逐阶段审批习惯 |
| RadishFlow / 1c0943bd | docs/adr/0001-branch-and-pr-governance.md、docs/development/agent-collaboration.md、.gitattributes | Rust 与宿主依赖隔离、稳定聚合；不迁入 CAPE-OPEN、COM、staging 或私有工程资料 |
| RadishAxiom / 4b53136 | docs/governance/repository-governance.md、agent-collaboration.md、Ruleset / workflow | 声明、自动检查、远端读回闭环；不迁入 IR、证明器或 macOS Rust runner |

## RadishRoot 特有约束

保留 no_std 与宿主隔离、unsafe / DMA 所有权、设备超时不等于 DMA 结束、UI / terminal 共用类型化服务、真实块设备写回、独立读盘、安装目标拒绝和任务资源清理。文档门禁无法证明这些行为；其验收仍由 MVP 的 FR / T / AT 负责。

## 后果

新增可运行的治理工具与失败测试需要维护，但不影响产品规划的任务顺序、需求编号或验收范围。后续规则变化同时更新文档、模板、检查器及远端；有本地资产不等于有 Git 历史或远端保护。首次引导及远端启用单独按实际授权推进。
