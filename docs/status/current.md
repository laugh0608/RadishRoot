# 当前状态

更新日期：2026-10-25（Asia/Shanghai）。

## 当前结论

**T01 已提交为 5b64eac；T02 本地实现与必要验证通过，新增修改尚未提交。** 已有真实 UEFI 开发镜像、串口、内存图 / framebuffer 校验和最小故障停止。产品文档仍为 2.0，第一 MVP、M0–M5、32 项任务和 24 项验收未变；T03–T32 未开始，M0 尚未通过，没有磁盘写回、正常关机、图形交互或安装能力。

| 层级 | 实际状态 |
| --- | --- |
| 依赖与身份 | 两成员 workspace；nightly-2026-10-13 / x86_64-unknown-none；limine 0.6.5、uart_16550 0.9.0、bitflags 2.13.2 锁定；Limine 12.9.4 与固定 QEMU / 固件 / 工具镜像配对通过 |
| 本地 Rust | fmt、宿主 / 裸机 check 与 clippy（含故障 feature）、16 项 xtask + 4 项纯引导契约测试通过；dev / release 新目录链接与独立 ELF 复核通过 |
| 真实 guest | 正常 release 镜像连续 10 次从新 VARS 启动；每次 32 段内存图、内核保留区域和 1024×768×32 framebuffer 事实通过。4 个受控注入用例均输出对应错误并 halt，无 READY |
| 独立读回 | 5 张专用镜像的主 / 备 GPT CRC、分区布局、FAT、8 个 ESP 文件及空 DATA 通过；正常盘启动前后摘要相同。最终重建的正常 / 故障 ELF 与 VM 用例字节一致 |
| 验收边界 | T02 门槛通过；AT01 取得当前候选的 10 次启动与无效输入证据，负向采用明确标识的生产校验路径注入，未测试真实旧版 bootloader。宿主终止 QEMU 不代表 guest 正常关机；AT23 / M0 未通过 |
| 仓库 / CI | 仓库检查及 14 项检查器回归通过；根入口逐字一致。CI install 补齐锁文件依赖获取，verify 仍复用本地入口；Ubuntu Actions 和 VM CI 未执行 |
| Git | dev；T01 提交为 5b64eac，领先本地 origin/dev 1 个提交；T02 留在工作树，暂存区为空。未 fetch / push / PR / 发布，远端实时状态未刷新 |

## 可执行入口与证据

- 使用方式见[开发规范](../development/standards.md#t02-镜像与串口入口)，范围见 [MVP T02](../mvp-plan.md#t02-本批实施范围与验收)。既有 check / build / test --host 继续有效；新增 `image`、`run --serial`、`test --vm`。首次依赖 / 资产准备与长时运行仍按明确授权执行，日常构建锁定且离线。
- 本批证据：`evidence/t02-boot-20261025-5b64eac/`，包含独立 GPT / FAT / 串口与 ELF 复核、源码摘要、日志 / ELF 副本和 SHA256SUMS。完整 VM 运行目录为 `target/xtask-80390-1792905921433080000/`；失败尝试另行保留，不算通过。候选身份为 HEAD 加源码内容摘要，不能只引用 5b64eac 冒充 T02 已提交。
- 具体结果见[环境清单第 2.11 节](../development/environment.md#211-t02-引导与失败验收)及[本月日志](../devlogs/2026-10.md)。T01 历史证据仍保留。当前只验收固定 macOS Homebrew / TCG 环境，不外推真机、其他固件 / QEMU 或 GUI。

既有远端治理本轮未修改或重新读回：master 为默认稳定主线，只经 PR 晋级；允许 merge / rebase，禁用 squash；master Ruleset 25073496 要求严格最新的 Candidate Quality（Actions App 15368），dev 无保护且普通 push 不触发 CI。此前治理 Actions 与 PR #1 未覆盖新增 Rust / T02 实现；流程见[Ruleset 手册](../../.github/rulesets/README.md)。

## 接下来的事项与停止线

1. **审阅 T02 本地交付。** 依后续授权处理提交、push、PR 与实际 Linux Actions；这些动作没有并入本批。首次远端运行记录工具安装、依赖获取、宿主测试与聚合结果。
2. **下一项为 T03 最小内存、PCI 与 DMA 所有权。** 先按既有 T03 条目细化从真实内存图选择可用页、保留区域与 DMA 生命周期，再接 PCI / BAR；必要新依赖在精确版本与影响明确后引入。不得回收正在使用的引导映射或把超时视为设备已停止访问。
3. **随后推进 T04–T05 的真实存储闭环。** 本次空 DATA 和只读开发盘不构成内核文件系统 / 写回能力，不跳过 flush、重启读回与宿主独立校验。

**本批停止线：**已完成用户授权的 T01 提交与 T02 开发 / 验证，在 T02 收口；不自动进入 T03、推送或发布。每周投入预算尚未提供，不承诺后续日历排期；原创许可仍待选择。

**保留资源：**忽略目录保留隔离 Cargo 缓存、构建输出、稀疏测试盘、固件变量副本及证据。任务 QEMU 和容器已回收，无新增后台服务；既有 Docker 服务 / 工具镜像及用户其他资源保留。完整成功套件共 5 张 1 GiB 逻辑盘，实际占盘另见证据，不是 5 GiB 全量写入。
