# 当前状态

更新日期：2026-11-09（Asia/Shanghai）。

## 规划阅读链

[产品整体规划](../product-plan.md) → [MVP 五个大专题](../mvp-plan.md#capability-map) → [安装与系统生命周期](../mvp-plan.md#installation-lifecycle) → [T13 M1 验收入口](../mvp-plan.md#t13-m1-演示入口与首轮真实验收)。任务状态、FR / AT 与完成规则以 MVP 原任务为准。

## 当前结论

**T13 进行中。自动主路径和 Cocoa 两窗人工演示通过，AT05、AT06 与 AT07 已在各自固定候选上通过 M1 范围验收。** 见[自动演示](../development/environment.md#225-t13-m1-自动演示首轮验收)、[Cocoa 验收](../development/environment.md#228-t13-cocoa-按窗口职责验收)、[AT06 图形补验](../development/environment.md#230-t13-at06-图形终端补验)和[AT07 交互验收](../development/environment.md#231-t13-at07-工作台交互验收)。

| 层级 | 实际状态 |
| --- | --- |
| Git 与 CI | [PR #3](https://github.com/laugh0608/RadishRoot/pull/3) 的 Candidate Quality 通过，已以 `c55ff01` 合入 master 并回流。origin/master 与 origin/dev 均指向该提交，祖先关系已复核。T13 自动演示实现与首轮验收记录已在本地 dev 提交为 `408bac8`。 |
| 入口 | [`xtask demo`](../development/standards.md#t13-m1-自动演示入口) 运行普通 release 内核。首启创建 `hello.txt` 后自动刷新，详情与写入同 ID，同名被拒。两次 QMP `guest-shutdown` 均自然退出为 0。宿主独立读回 16 字节，次启只读打开同一 DATA，内容一致，旧操作返回 `ForeignBoot`，整盘摘要一致。 |
| 故障对照 | `t12-volume-fail` 用独立 DATA 注入卷同步失败，界面保留原阶段与原 ID，后续写入被拒，诊断仍可执行。宿主回收该进程，独立读盘通过。 |
| 人工入口 | [`xtask demo --gui`](../development/standards.md#t13-cocoa-人工演示入口与清单) 以 `45a8903` 为基线按窗验收。首窗通过实体鼠标、焦点和冲突，次窗通过键盘、同盘只读读回、原 ID 的 `ForeignBoot` 拒绝与 Shift 冒号。两窗自然关机及 10 帧像素检查通过。实现和记录已在本地 dev 提交为 `b8974ff`。 |
| AT06 | 宿主与真实串口 guest 的 2151 字节输入相同，8 条结构化回复和 2 条超限通知逐项一致。独立读盘核对了唯一文件、精确 1002 字节、FAT 结构及 DATA 外字节。实现和记录已在本地 dev 提交为 `11e91f9`，候选与资源见[环境清单第 2.29 节](../development/environment.md#229-t13-at06-宿主与串口-guest-等价)。 |
| 图形补验 | `test --vm --workbench-equivalence` 在真实 QEMU 通过。guest 解码字节与同一宿主夹具一致，8 条命令回复匹配。超限提示、Enter 清空、后续读回、4 帧像素及独立读盘均通过。图形实现和记录已在本地 dev 提交为 `1c4c795`。 |
| AT07 工作台 | `test --vm --workbench-layout` 在真实 QEMU 通过。鼠标和 Tab / Enter 完成单双面板切换、详情与刷新，六帧独立像素验证和宿主重放一致，只读整盘摘要一致。实现和记录已在本地 dev 提交为 `bcab8dd`。 |
| M1 空闲测量 | 真实 QEMU 的 100 个空闲输入样本 P95 为 18.500 ms，低于 150 ms。已记录主机负载、内核保留范围、堆、framebuffer、DMA 和 UI 预算及采样口径。实现和记录保存在工作区。见[环境清单第 2.32 节](../development/environment.md#232-t13-m1-空闲输入与内存测量)。 |
| 本地验证 | M1 测量候选通过 116 项宿主测试、fmt、宿主和裸机 check / 严格 Clippy、dev / release 链接及测量内核链接。宿主信息查询调整后复核了 fmt、check 和严格 Clippy。 |
| 未验证面 | 阶段候选收口复核及复制负载测量。 |

## 接下来的事项

1. 核对 M1 内部预览候选和各项验收的适用范围，按 [T13 通过条件](../mvp-plan.md#t13-m1-演示入口与首轮真实验收) 列出需要重跑的证据，先处理 P0 / P1。
2. 阶段候选复验按影响与资源另列。复制负载随 T25 / AT18 验证，收口时核对第 9 节的测量要求。

## 已确认范围与停止线

- 首轮批准最多 6 次无窗口 VM、10 张专用稀疏 raw，单次最多 60 秒，盘及 FAT 分配 512 MiB、其他产物 2 GiB，总执行 15 分钟。实际 3 次、5 张 raw、9 份 FAT，盘及 FAT 约 47.5 MiB，其他约 198.6 MiB，运行窗口 74 秒。进程、容器与私有 socket 已核对回收，保留盘、日志与帧。后续运行另定预算。
- Cocoa 一轮额度已结束。批准两次顺序启动、每窗 300 秒，2 张专用稀疏 raw，盘及 FAT 分配 256 MiB、其他产物 1 GiB，总执行 15 分钟。实际首窗超时返回 124，使用 2 张 raw、3 份 FAT，盘及 FAT 约 18.5 MiB，其他约 112.5 MiB，运行窗口 314 秒。进程、容器和私有 socket 已核对回收，保留失败盘与证据。
- Cocoa 复验按同额度执行一次。实际两窗、2 张 raw、4 份 FAT，盘及 FAT 约 22.5 MiB，其他约 153.8 MiB，整轮 368 秒。任务进程、容器和私有 socket 均已回收，保留盘、帧与日志。
- 按窗口职责的 Cocoa 验收按同额度执行一次。实际两窗、2 张 raw、4 份 FAT，盘及 FAT 约 22.5 MiB，其他约 154 MiB，整轮 433 秒。任务进程、容器和私有 socket 均已回收，保留盘、帧与日志。
- AT06 无窗口额度已用完。实际 1 次启动、2 张 raw、3 份 FAT，盘及 FAT 约 18.5 MiB，其他约 188.4 MiB，VM 含回收约 29.7 秒，整轮约 46 秒。任务进程、容器和私有串口已回收，保留盘、候选和日志。
- AT06 图形补验额度已用完。实际 1 次无窗口启动、2 张 raw、3 份 FAT，盘及 FAT 约 18.5 MiB，其他约 191.7 MiB，VM 含回收约 19.3 秒，整轮约 40.2 秒。任务进程、容器和私有 socket 已回收，保留盘、截图、候选和日志。
- AT07 工作台额度已用完。实际 1 次无窗口启动、1 张只读 raw、2 份 FAT，盘及 FAT 约 6.5 MiB，其他约 108 MiB，VM 含回收约 4.7 秒，整轮约 21.5 秒。任务进程、容器和私有 socket 已回收，保留盘、帧、候选和日志。
- M1 空闲测量额度已用完。实际 1 次无窗口启动、1 张只读 raw、2 份 FAT，盘及 FAT 约 6.6 MiB，其他约 205.6 MiB。VM 含回收约 8.4 秒，从首次执行到测量返回约 129.5 秒。任务进程、容器和私有 socket 已回收，保留盘、样本和日志。
- T12 专用预算、既有盘、VARS、日志及资源退出记录见[环境清单第 2.24 节](../development/environment.md#224-t12-正常同步与电源验收)。原创许可与每周投入仍待所有者决定。
