# 仓库治理

本文定义长期 Git / PR / CI 策略；理由见 [ADR 0002](../adr/0002-repository-foundation.md)，实际启用情况见[当前状态](../status/current.md)。本地声明、检查执行和远端强制是不同事实。

## 1. 规则与资产职责

用户明确任务决定授权范围；产品 / MVP 定义能力边界；ADR 记录长期决定；专题维护完整规则；current 维护当前事实；AGENTS / CLAUDE 提供启动约束与路由。自动化实施规则，不自行创造产品要求。发现文档与实现不符，先定位过期方，再在同一变更中修正。

| 资产 | 职责 |
| --- | --- |
| AGENTS / CLAUDE、协作专题 | 启动约束与任务 / 资源边界；两根入口逐字一致 |
| CONTRIBUTING、CODE_OF_CONDUCT、SECURITY | 贡献入口、行为边界与私密安全报告 |
| .editorconfig / .gitattributes / .gitignore | 编码、换行、二进制与生成物边界 |
| .github 的 PR / Issue 模板 | 目标、复现、证据、风险与回流责任 |
| scripts/check-repo.py、scripts/tests | 本地与 CI 共用的真实检查及拒绝回归 |
| .github/workflows/pr-check.yml | 组件检查与稳定聚合结果 |
| .github/rulesets/ | 远端声明及启用 / 读回步骤 |

不建立装饰性 CODEOWNERS、虚假 LICENSE、空 Cargo 工程或自动发布入口。原创许可由所有者另行决定，见[许可与来源](licensing-and-sources.md)。

## 2. 分支与提交

| 分支 | 进入方式与职责 |
| --- | --- |
| `dev` | 单维护者日常开发与集成，普通任务本地验证后直接提交；无强制 PR / required checks |
| `master` | 目标 GitHub 默认分支和稳定主线，只接受阶段晋级 PR 或必要 hotfix PR |
| 主题分支 | 用户要求、外部贡献、并行写入或确有隔离收益时建立，通常 PR 到 dev |

主题可用 feature/、fix/、docs/、chore/、experiment/、hotfix/；Agent 使用自身前缀仍遵守同样路由，不自动创建分支或 worktree。日常贡献显式选择 dev 为 PR base；默认分支为 master 不表示日常向它开发。

提交使用用户身份、Conventional Commits 和中文说明，例如 `docs(governance): 完善主线回流规则`。类型限 feat、fix、docs、style、refactor、perf、test、build、ci、chore、revert，允许可选 scope 和 `!`。真正的多父 merge commit 不强制改写标题；单父提交不能以伪装 `Merge ...` 绕过。新 revert 提交标题使用 `revert: ...`。

只暂存本批精确路径，复核 staged diff、未跟踪文件、敏感材料和文件大小；不广泛 add、不顺手提交用户改动。提交按任务授权，不自动包含 push / PR / 发布。禁止 rebase、reset、force push 等共享历史重写。

## 3. 合并与回流闭环

允许 merge commit / rebase merge，禁用 squash；阶段性 dev → master 优先 merge commit。rebase merge 产生新 SHA，但不能再 rebase 共享 dev 来追随它。不要启用 require linear history；不要自动删除长期 dev。

PR 写最终行为、实际证据、未验证项、风险与恢复方式。面向 master 时指定回流执行人和时机。单维护者审批数为 0，不要求自我审批或无人的 CODEOWNERS，review conversations 仍须解决。base / head 变化及冲突解决后补相应验证。

任何 PR 合入 master 后，暂停 dev 下一轮提交，按以下顺序闭环（需已有对应远端操作授权）：

1. 确认工作区无会被覆盖的改动，fetch 最新远端，切到 dev 并核对与 origin/dev 的关系。
2. 本地 dev 落后时 `git pull --ff-only origin dev`；分叉时先审查双方提交，不覆盖任一方。
3. **dev 是 master 的祖先**时 `git merge --ff-only origin/master` 快进；master 已是 dev 祖先时无需合并；真正分叉时普通 `git merge origin/master`。
4. 冲突逐项复核，内容变化做风险匹配验证；纯拓扑同步且树不变，不重复全量测试。
5. 推送 dev，重新 fetch；`git merge-base --is-ancestor origin/master origin/dev` 返回 0 才证明远端 dev 包含最新主线。若 master 又变化，补同步后再继续开发。

不能只用 SHA 相等判断：普通 merge 后 SHA 可不同，但 ancestry 必须成立。hotfix 同样回流。未经授权不为收口擅自 push，交接明确本地 / 远端完成情况。合并与回流不代表 tag、Release 或部署。

## 4. CI 与 Ruleset

普通 push（含 dev）、tag 不自动触发 CI；面向 dev / master 的 PR 与手动调度运行 PR Checks。不使用 paths 过滤，避免 required context 永不产生；不开自动合并、部署、Release 或 merge queue。

当前组件 Repo Hygiene 执行真实治理检查及检查器回归；Rust Checks 读取项目工具链配置，执行宿主 / 裸机检查、宿主测试与 dev / release 干净链接。Candidate Quality 通过 always() 等待这两个组件；结果集合必须精确等于 repo-hygiene / rust-checks 且每项均为 success 才通过，failure、cancelled、skipped、缺失结果或未知组件均失败。Ruleset 只绑定这个稳定 context。三个 job 的超时基线分别为 5 / 15 / 2 分钟；组件变化同时更新 workflow needs 和本地契约，不为不存在的业务测试建立空成功 job。

PR 检查 GitHub 的候选 merge checkout，获取完整历史供 diff / commit range 检查；token 只读、不持久化凭据，不以 pull_request_target 执行贡献者代码。只用 GitHub hosted runner，不将外部 PR 放在宿主自托管 runner。checkout 固定上游 commit；Python 使用 runner 现有版本，不等于产品工具链 pin。workflow 使用 JSON 语法的 YAML 文件，标准库可严格解析，无需 YAML 依赖。

master 目标保护：禁止删除 / 强推、必须 PR、解决讨论、严格最新的 Candidate Quality、0 审批；管理员仅 PR 内可 bypass。例外需明确授权并记录原因，不能把绕过写成检查通过。integration_id 在首次真实运行后读取绑定，不猜测 App ID。步骤与平台限制见 [Ruleset 运维说明](../../.github/rulesets/README.md)。

## 5. 可执行检查及边界

本地：`python3 scripts/check-repo.py`；回归：`python3 -B -m unittest discover -s scripts/tests -v`。Python 3.10+ 标准库即可，不联网、不安装依赖、不运行 VM。PR 额外传 `--base-ref <base SHA>`，检查 merge-base 到 HEAD 的提交标题与完整 diff。无 Git 指定 base、非法 ref 或 Git 失败都报错。

检查覆盖必需文件、根入口一致、UTF-8 / BOM / LF / 末尾换行 / 尾随空白、JSON、Markdown 本地文件链接、路径 / 单文件体积、workflow 与 Ruleset 契约。有 Git 时枚举已跟踪及未忽略未跟踪文件，检查工作区 / 暂存区 diff 空白；强制跟踪的忽略产物仍拒绝。无 Git 时明确报告文件模式，按本地忽略边界扫描，不冒称提交或分支验证通过。

默认相对路径上限 180 字符、单文件 10 MiB。小型合成 fixture 确需例外时在任务中说明并调整具体规则，不用 force add 绕过。秘密扫描不在文本检查能力内；链接只检查目标文件存在及不越出仓库，不验证网页、锚点或完整 Markdown 语法。暂存内容业务正确性仍须提交前 staged diff 审查。

Rust 工程已接入真实 fmt / check / clippy / tests、no_std 目标链接；存储 / DMA / 电源接真实 guest 与独立读盘；UI 和安装补交互与介质移除证据。实现新增时同步扩展验证，不能长期用文档 CI 代表内核质量。

## 6. 同步与首次引导

| 改动 | 同步维护 |
| --- | --- |
| 分支 / 合并策略 | 本文、ADR、PR 模板、Ruleset 声明与说明 |
| required context / 组件 / 权限 | workflow、检查器及回归、Ruleset、本文 |
| 协作规则 | 专题，必要时两份根入口 |
| 许可 / 安全报告 | 许可策略、CONTRIBUTING、SECURITY、实际报告入口 |
| 范围 / 格式 / 平台 | 产品 / MVP 或最近专题及验收 |
| 当前能力 / 远端启用 | current 与月度 devlog |

初始化 Git、首次 commit、远端创建 / 可见性、push、Rulesets 与发布按相应授权执行。首次引导可在保护启用前建立一次共同基线提交及 master / dev，必须留痕，不扩张为后续直接更新主线的惯例。已有仓库不重建历史。远端缺权限或套餐能力时记录阻塞，不静默降级规则或更改公开性。
