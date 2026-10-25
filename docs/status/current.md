# 当前状态

更新日期：2026-10-25（Asia/Shanghai）。

## 当前结论

**T02 已提交为 63e2193；T03 本地实现与必要验证通过，修改尚未提交。** 已有真实 UEFI 引导、物理页 / 固定堆、PCI 发现、BAR 映射及 VirtIO HAL。产品文档仍为 2.0，第一 MVP、M0–M5、32 项任务和 24 项验收未变；T04–T32 未开始，M0 / AT03 尚未通过，没有磁盘读写、flush、正常关机、图形交互或安装能力。

| 层级 | 实际状态 |
| --- | --- |
| 依赖与身份 | 两成员 workspace；nightly-2026-10-13 / x86_64-unknown-none；保留 T02 的 3 个外部包，按授权新增 19 个精确包，共 22 个 registry 包；x86_64、linked_list_allocator、virtio-drivers 仅用于裸机 target，xtask 仍只用 std |
| 本地 Rust | fmt、宿主 / 裸机 check 与严格 clippy（含故障 feature）、17 项 xtask + 11 项纯契约测试通过；dev / release 全新链接与独立 ELF 复核通过 |
| 真实 guest | 正常 release 连续 10 次从新 VARS 启动；页 / 堆读写及释放记账、真实 VirtIO-blk 发现、UC BAR 映射、HAL 分配 / bounce CPU 拷贝通过；4 个 T02 和 3 个 T03 受控故障均停止 |
| 独立读回 | 8 张专用盘的主 / 备 GPT CRC、FAT、每盘 28 个 ESP 文件及空 DATA 通过；正常盘启动前后摘要一致，独立构建 release 与 VM 内核字节相同 |
| 验收边界 | T03 门槛通过。bus master 禁用、无队列或设备 IO；DMA 超时是生命周期状态注入，验证拒绝释放和隔离，不是实际设备超时。AT03、AT23 / M0 仍待后续真实 IO、持久化和完整交付；宿主终止 QEMU 不代表 guest 正常关机 |
| 仓库 / CI | 仓库检查及 14 项检查器回归通过；根入口逐字一致。本机 ci-rust.py verify 通过；Ubuntu Actions 和 VM CI 未执行 |
| Git | dev；HEAD 为 63e2193，领先本地 origin/dev 2 个提交；T03 修改留工作树，暂存区为空。未 fetch / push / PR / 发布，远端实时状态未刷新 |

## 可执行入口与证据

- 使用方式见[开发规范](../development/standards.md#t02-镜像与串口入口)，范围见 [MVP T03](../mvp-plan.md#t03-本批实施契约)。沿用 check / build / test --host、image、run --serial、test --vm；VM 成功终态已推进到 T03 READY，T02 READY 只是中间阶段。依赖 / 资产准备与长时运行按明确授权执行，日常构建锁定且离线。
- 本批证据：`evidence/t03-platform-20261025-63e2193/`，包含独立 GPT / FAT / 串口与 ELF 复核、源码摘要、日志 / ELF 副本和 SHA256SUMS；依赖与许可原始核对在 `evidence/t03-preflight-20261025-63e2193/`。完整 VM 目录为 `target/xtask-4531-1792908412840199000/`。候选身份是 HEAD 加源码摘要，不能只引用 63e2193 冒充 T03 已提交。
- 结果见[环境清单第 2.12 节](../development/environment.md#212-t03-内存pci-与-dma-边界验收)及[本月日志](../devlogs/2026-10.md)。T01 / T02 历史证据保留；当前只覆盖固定 macOS Homebrew / TCG 环境，不外推真机、其他固件 / QEMU 或 GUI。

既有远端治理本轮未修改或重新读回：master 为默认稳定主线，只经 PR 晋级；允许 merge / rebase，禁用 squash；master Ruleset 25073496 要求严格最新的 Candidate Quality（Actions App 15368），dev 无保护且普通 push 不触发 CI。此前治理 Actions 与 PR #1 未覆盖新增 Rust / T02 / T03 实现；流程见[Ruleset 手册](../../.github/rulesets/README.md)。

## 接下来的事项与停止线

1. **审阅 T03 本地交付。** 后续提交、push、PR 与实际 Linux Actions 按对应授权处理；首次远端运行记录工具安装、依赖获取、宿主测试与聚合结果。
2. **下一项为 T04 块读写、flush 与 GPT 边界。** 先细化队列 / 请求所有权、完成与复位证据、有界轮询和超时隔离，再开启 bus master。现有 HAL 适配不能直接当作已验收的块驱动；上游无界等待及 Drop 必须逐项处理。
3. **T04–T05 补齐真实存储闭环。** 只读开发盘和空 DATA 不构成文件系统 / 写回能力；必须取得 flush、重启读回与宿主独立校验，不复用当前 CPU 拷贝证据替代设备 DMA。

**本批范围：**已按用户要求提交 T02 并完成下一项 T03；精确依赖 / lockfile 与 VM 验证均有明确授权。本批停止在 T03，不进入 T04，不追加提交、推送或发布。每周投入预算尚未提供，不承诺后续日历排期；原创许可仍待选择。

**保留资源：**忽略目录保留隔离 Cargo 缓存、构建输出、稀疏测试盘、VARS 及证据。任务 QEMU 和容器已回收，无新增后台服务；既有 Docker 服务 / 工具镜像及用户资源保留。成功套件共 8 张 1 GiB 逻辑盘，实际分配 6,422,528 字节；首次单次启动盘及历史材料另行保留，不是 8 GiB 全量写入。
