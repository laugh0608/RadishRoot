# 当前状态

更新日期：2026-10-14（Asia/Shanghai）。

## 当前结论

RadishRoot 已建立产品文档、通用仓库治理基线及远端保护。产品文档仍为 2.0：第一 MVP、M0–M5、32 项任务和 24 项验收未变。T01 已开始环境准备，完成本机盘点、安装候选文档及 Debian 签名策略 / 模拟求解验证，尚未完成可编译工作区；T02–T32 未开始。没有 Cargo 工程、内核、产品镜像或产品运行入口；不能启动系统，没有业务验收证据。

| 层级 | 实际状态 |
| --- | --- |
| 本地治理检查 | 本批 31 个文件的仓库检查通过；检查器既有 13 项回归通过证据见月度日志，本批未改检查器、不重复运行；两份根入口逐字一致 |
| T01 环境 | 稳定版 Rust 1.96.0 / 1.97.1 已有，裸机 target / nightly / 独立 QEMU CLI 缺失；UTM 内有固件样本；现有 OrbStack Docker 服务只读查询成功 |
| 安装候选 | Debian 日期标签 → ARM64 digest 已核对；QEMU 11.1.2 的 30 项制品候选身份已记录，选定 libssh 0.12.2 / OpenSSL 3.6.4_1；30 项配方来源 / 哈希和静态依赖图已核对；Debian 固定容器内 sqv / APT 签名、快照过期拒绝及五包模拟求解通过；未安装工具包，QEMU 实际事务 / 运行未验证，见[环境清单](../development/environment.md) |
| 安装影响 | 拟采用专用 `/opt/rrt01`，QEMU 集合 30 项全部专用新增，0 项共享升级 / 重装；30 项元数据满足短前缀重定位条件，尚未实际安装；Debian 无 recommends 实际模拟为新增 5 包、0 升级 / 移除；未安装 |
| Git / 远端 | 已连接 [laugh0608/RadishRoot](https://github.com/laugh0608/RadishRoot)；master 为稳定主线，dev 为日常开发分支；治理晋级与回流的具体提交及检查证据保存在对应 PR |
| 默认分支与合并 | 默认 master；允许 merge / rebase，禁止 squash；关闭自动合并与自动删除 head branch |
| master Ruleset | [25073496](https://github.com/laugh0608/RadishRoot/rules/25073496) 为 active，仅覆盖 master；要求 PR、解决讨论、严格最新的 Candidate Quality；禁止删除 / 强推；0 审批，管理员仅 PR 内 bypass |
| 检查来源与权限 | Candidate Quality 绑定实测 GitHub Actions App 15368；工作流默认只读，不能审批 PR |
| dev 保护 | 有效 Rulesets 为空，传统保护返回未保护；允许按风险本地验证后直接提交 / push |
| 实际 Actions | 手动首跑成功；PR 故障候选阻断，修复候选通过；普通 dev push 不触发自动 CI |
| 产品验证 | 已读取工具版本；项目 Rust 编译 / 宿主交互 / QEMU guest / 读盘 / 安装均未执行；T01、AT01、AT23 均未通过 |

## 验证入口与证据

- 本地：`python3 scripts/check-repo.py`；回归：`python3 -B -m unittest discover -s scripts/tests -v`。PR 使用 `--base-ref` 检查提交范围。
- [master 手动首跑](https://github.com/laugh0608/RadishRoot/actions/runs/38557500889)：Repo Hygiene 与 Candidate Quality 均成功，核对运行 SHA 和 Check Run App ID。
- [验证 PR #1](https://github.com/laugh0608/RadishRoot/pull/1)：[故障运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557855601)两组件失败、PR blocked；[修复运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557986984)两组件通过、PR clean。PR 已关闭且未合并，临时验证分支已删除。
- 已读回默认分支、合并选项、Ruleset 详情、master / dev 有效规则及 Actions 权限。未对真实 master 做删除、直接 push、force push 或 bypass 演练。
- [月度日志](../devlogs/2026-10.md)记录配置、失败 / 成功证据、Git 动作和限制；[Ruleset 手册](../../.github/rulesets/README.md)维护操作流程。

治理证据只证明仓库门禁；环境盘点只证明已观察到的工具与元数据。现有 checkout pin 在 runner 上产生 Node.js 20 被强制转为 Node.js 24 的兼容提示，检查已成功；后续工具维护时专项评估。安全报告私密入口未启用或验收，仍按 SECURITY 指引处理。

## 下一项与停止线

继续在 dev。前批隔离方案已提交并推送为 1889365；本批已获授权拉取固定 Debian digest、启动任务专用临时容器，只做签名 / 过期负向检查与五包模拟求解，清理容器后同步文档、验证并提交 / 推送 dev。不直接推送 master，不创建 PR、产品 Release 或部署。

Debian 验证完成，见[环境清单第 2.5 节](../development/environment.md)：三个签名通过、六个签名负向样本拒绝，APT 更新与索引匹配通过；去掉局部有效期例外时两个历史源均被拒绝；模拟结果为五包新增、零升级 / 移除。78 项基础包清单及 dpkg 状态未变，没有下载 .deb。两个任务容器已删除，固定镜像缓存保留，无后台资源。

下一项仍属环境实施：QEMU 专用前缀 / 固定 tap、真实依赖解析、重定位与恢复验证均未执行；Debian 工具包实际安装及工具镜像也未建立。按相应授权推进，不能以本次模拟通过代表工具已可运行。Cargo workspace、业务代码与 guest 运行另按实际授权推进。

本轮停止线：仅完成获准的 Debian 验证；不安装工具包或其他依赖，不实施 QEMU / nightly 安装，不启动 guest VM，不写业务代码或生成工具链锁文件。原创许可、实际编译依赖与功能开关、Limine 协议配对、Live 来源及性能预算尚未冻结；实施排期前记录每周投入，94 人日仍是估算。T01 完成须有干净目录的最小 no_std 编译与依赖隔离证据，M0 退出还须 T02–T05 的真实存储证据。
