# master Ruleset 声明与启用

[master-protection.json](master-protection.json) 是目标配置，active 表示导入后预期生效，文件本身不连接 GitHub。策略正式源是[仓库治理](../../docs/governance/repository-governance.md)；本模板不携带仓库名、Secrets 或规则 ID。

## 声明范围

仅匹配 refs/heads/master，不匹配 dev 或 tag；四项规则为 deletion、non_fast_forward、pull_request、required_status_checks。0 审批、解决讨论、允许 merge / rebase、禁止 squash；RepositoryRole 5（管理员）只在 PR 内可绕过。不开 branch creation 的检查豁免，先建立初始分支和检查，再启用保护。

不要求 linear history、签名或 CODEOWNERS，不预建 merge queue、发布 tag 或推送型 Ruleset。提交标题由 PR 检查验证，避免远端正则阻塞正常 merge commit。

## 启用顺序

这是操作手册，不是执行授权；实施前先确认目标 owner/repository、可见性、套餐、权限和副作用。

1. 读取仓库设置、分支、Rulesets（含继承规则）及传统 branch protection，保存实际配置。已有同范围规则按 ID 更新，不重复 POST 叠加。
2. 获得初始化 / 提交授权后建立共同基线和 master / dev；远端授权后推送。目标默认分支 master，日常工作切回 dev。不修改全局 Git 身份、不自动公开仓库。
3. 允许 merge commit / rebase merge、关闭 squash；关闭自动删除 head branch，避免删除长期 dev。Actions 使用只读权限，不启用 workflow 自批自合。
4. 在真实候选 PR 或手动运行中取得 Candidate Quality，核对 SHA、组件结果、触发类型和 Check Run 的 App ID。首次 PR 也须观察失败组件阻断聚合，验证普通 dev push 不触发 CI。
5. 将真实 App ID 写入 required_status_checks 项的 integration_id，同步本地声明后启用。模板未绑定时只检查名称，不能声称来源已受保护，不照抄兄弟项目的 ID。
6. 通过 Settings 或 REST API 导入 / 更新；核对 active、refs、strict、context / integration_id、PR 条件、bypass 和仓库合并选项。
7. 读回设置、Ruleset 详情及 master 的有效规则，结合 PR 状态验收。不要对真实 master 尝试删除、直接 push 或 force push；需要破坏性演练时另行授权一次性仓库。
8. current 分别记录声明、本地检查、Actions 首跑和远端强制状态，devlog 留实际 ID、SHA 与证据链接，不存 token。每次主线合并仍须回流 dev。

只读 API：`GET /repos/{owner}/{repo}`、`GET /repos/{owner}/{repo}/rulesets?includes_parents=true`、`GET /repos/{owner}/{repo}/rulesets/{id}`、`GET /repos/{owner}/{repo}/rules/branches/master`；传统保护另读 branches/master/protection。创建用 POST rulesets，更新用 PUT rulesets/{id}，不提供自动写远端脚本。

变更失败时保留错误与读回结果。恢复依据先前快照精确更新，再读回确认；不通过删除全部规则、扩大 bypass 或关闭检查来制造绿色。

## 官方依据

2026-10-14 核对：Ruleset 可用性受可见性和套餐影响，多项适用规则共同生效。PR-only bypass 要求经 PR 留痕。实际账号启用前再次确认。[GitHub Rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets)、[创建与 bypass](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository)。

字段、integration_id 与有效规则查询见 [Rules REST API](https://docs.github.com/en/rest/repos/rules)；触发与 needs 见 [Actions 语法](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)。checkout 固定上游 [v4.2.2](https://github.com/actions/checkout/releases/tag/v4.2.2) commit，升级须复核来源与运行要求。
