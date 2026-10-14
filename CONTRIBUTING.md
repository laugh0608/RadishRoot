# 参与开发

先阅读 [AGENTS.md](AGENTS.md)、[当前状态](docs/status/current.md)和任务对应的 [MVP 规划](docs/mvp-plan.md)。产品与实施状态严格区分；未经实施授权，不因文档中存在任务而自动创建代码或安装依赖。

1. 查看工作区与现有修改，确定一项可完整验证的任务。
2. 按[开发规范](docs/development/standards.md)实现真实主路径与必要失败路径，更新最近正式来源。
3. 完成范围匹配的检查；附实际目标、版本、结果与未验证项，不贴敏感数据或无界日志。
4. 按[仓库治理](docs/governance/repository-governance.md)组织 Conventional Commits；外部贡献使用主题分支并优先面向 `dev`。
5. 提交 / push / PR / 发布各自按实际授权执行；阶段晋级后先回流 `dev`。

不为单纯文案改动要求所有业务测试；重要行为不能只用编译或文档检查证明。不得把真实个人数据、虚拟机大镜像、依赖缓存、第三方参考截图混入提交。

原创许可和对外贡献授权机制尚未选定，相关边界见[许可与来源](docs/governance/licensing-and-sources.md)。

仓库检查运行 `python3 scripts/check-repo.py`，检查器变更另跑 `python3 -B -m unittest discover -s scripts/tests -v`；无 Git 时只检查文件并明确提示，不跳过文档检查。实际产品行为仍按开发规范取得证据。

普通贡献 PR 显式以 `dev` 为目标；目标默认分支 `master` 只收阶段晋级或必要 hotfix。PR 使用仓库模板，主线合并后完成回流再继续开发。安全问题遵循 [SECURITY.md](SECURITY.md)，讨论与评审遵循[行为规范](CODE_OF_CONDUCT.md)。这些入口不代表已开放公共仓库或已选择开源许可。
