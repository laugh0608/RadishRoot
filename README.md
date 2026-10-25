# RadishRoot（萝卜根系）

> 看见状态，理解操作。

RadishRoot 是以可解释操作与结构化交互为核心的 Rust 实验操作系统。它从固定虚拟机开始，提供共享真实文件服务的文件管理器、结构化终端和操作详情，让资源状态、执行阶段、错误原因与部分修改都可以被查看。

第一阶段使用现成引导、虚拟磁盘、分区与文件系统组件，聚焦内核适配、统一服务和简单图形工作台。当前实施状态与下一步只在[当前状态](docs/status/current.md)维护；规划中的能力不代表已有实现。

## 产品边界

- 在固定 QEMU x86_64 配置下独立启动和运行；首个 MVP 包含空白虚拟盘安装、文件操作、正常关机后的持久化与双应用联动。
- 可解释信息来自真实状态、结构化错误与执行记录；不依赖语言模型生成事实。
- 家族一致性体现在产品语义、开发方式和工作台视觉；不预设现有 Radish 应用可以直接运行于本系统。
- 真机、网络、多核、第三方应用、POSIX 兼容、完整桌面与中文输入法不属于第一 MVP。

## 文档入口

- [正式文档导航](docs/README.md)
- [完整产品规划](docs/product-plan.md)：定位、长期演进、家族关系与产品成功标准。
- [MVP 项目规划](docs/mvp-plan.md)：技术、架构、UI、范围、32 项任务和 24 项验收。
- [开发规范](docs/development/standards.md)：Rust、版本、验证、依赖与文档职责。
- [仓库治理](docs/governance/repository-governance.md)：dev / master、提交、PR、回流与 CI 原则。
- [协作规则](docs/governance/agent-collaboration.md)、[贡献指南](CONTRIBUTING.md)
- [范围与顺序决策](docs/adr/0001-product-and-mvp-boundary.md)
- [仓库底层决策](docs/adr/0002-repository-foundation.md)、[Ruleset 启用手册](.github/rulesets/README.md)

仓库文档与治理检查：`python3 scripts/check-repo.py`；检查器回归：`python3 -B -m unittest discover -s scripts/tests -v`。仅需已有 Python 3.10+，不安装依赖。检查通过不代表内核、真实虚拟机或安装交付通过。安全问题见 [SECURITY.md](SECURITY.md)。

构建、宿主检查及 T02–T04 镜像 / 串口与块存储运行入口见[开发规范](docs/development/standards.md#t01-本地入口)；实际能力和验证边界以当前状态为准。产品原创内容的许可尚未选定，见[许可与来源策略](docs/governance/licensing-and-sources.md)。
