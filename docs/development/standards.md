# 开发规范

沿用 Radish 系列的中文协作、明确类型、单维护者串行开发、单一正式来源与按风险验证。工程复杂度以真实问题为依据，不以文件、crate、测试或文档数量衡量完成度。

## 1. 日常推进与工程结构

先核对工作区，再读[当前状态](../status/current.md)和任务对应的[MVP 规划](../mvp-plan.md)。每轮优先完成可以复验的一条行为路径，交接写清启动入口、当前画面 / 能力、证据和下一项任务。

当前 Cargo workspace 包含 kernel、xtask 与复用共享视图的 host-runner；kernel / platform、system-core / services、shell / ui、host-runner、xtask 按职责演进。开始时可用少量 crate 内模块承载这些边界；出现不同目标依赖、独立测试或稳定复用价值时再拆 crate，不为架构图机械建空目录。

共享核心以 no_std + alloc 为基线；宿主窗口库和 std 后端只在 host-runner / 测试工具中。UI 调用类型化服务，服务通过明确存储边界访问 FAT；文件语义不在不同入口重复实现。

## 2. 代码与资源

- 使用清楚的领域命名、早返回、浅调用层和结构化错误；优先标准库 / 核心库与现有能力，不预建万能 manager、factory、插件或事件总线。
- 沿用现有分层与配置入口，不用字符串标记、隐式全局状态或动态技巧替代明确类型；共享接口 / 格式变化同时复核生产者、消费者、示例、测试及版本 / 兼容语义。
- FileEntry、VolumeInfo、MemoryStats、OperationRecord 等具体类型承担真实职责；公共投影不反过来抹平类型。
- 单文件接近 1000 行时评估职责，原则上不超过 1500 行；按同域职责拆分，不机械切片或增加无意义转发。
- 所有外部输入、地址与长度、整数运算、IO、超时、资源配额和格式边界显式验证。不能用空值或默认成功掩盖损坏。
- unsafe 局限于必要硬件 / 内存边界，注明安全前提、指针来源、物理 / 虚拟映射、对齐、别名、生命周期、并发与释放责任。
- 中断只采集事件；耗时服务在主循环按有界步骤推进。DMA 超时后先确认设备停止访问，不能按普通函数超时释放缓冲区。
- 日常可恢复错误返回 Result；panic 用于无法继续的内部错误并留下串口诊断，不跨 FFI 或回调边界失控展开。
- 新 fallback 必须有触发条件、可观察状态与停止线；不悄悄改写数据、降低持久化或隐藏原始失败。
- 时间用单调时钟表达阶段顺序；跨重启身份包含 boot_id，不能把时钟值当持久唯一 ID。

## 3. 依赖、工具链与版本

引入依赖前说明用途、替代方案、精确版本候选、no_std 特性、上游维护与许可；先实际验证再冻结。Git 依赖固定 commit，提交 Cargo.lock 和 rust-toolchain.toml；不得默认跟随主分支。更新 lockfile、联网下载和修改全局工具链按实际授权执行。

文档基线使用本文所属文档版本；MVP `v0.1` 表示产品能力目标。正式发行沿用家族日历版本 `vYY.M.RELEASE`，开发渠道可附 dev / test；首次工程落地时设计单一产品版本来源，再由构建同步 manifests / 镜像，不在仅文档阶段生成虚假发行标识。协议、磁盘格式、事件 schema 与产品版本分别管理；日历版本变化不替代兼容设计。

固定 QEMU、带版本机器名、OVMF、引导协议和镜像配置；实际支持范围按验证矩阵声明。宿主模拟器不是第二个产品平台。默认不以本机加速模式取代已选 TCG 验收基线。

## 4. 构建入口与检查

构建、镜像、运行、demo、host / VM 测试与安装验证通过同一 xtask 编排；shell 包装不另实现规则。每个入口随真实能力建立，说明参数、资源、时限、输出与错误码，不提供返回成功的空检查。

仓库治理保留独立真实入口 `python3 scripts/check-repo.py`，只依赖 Python 3.10+ 标准库；回归用 `python3 -B -m unittest discover -s scripts/tests -v`。它不运行 Rust 或 VM。未来迁入 xtask 时保留单一实现，不在 Python 与 Rust 中复制两套政策。检查范围及限制见[仓库治理](../governance/repository-governance.md)。

开发工具环境使用 [tools/debian-image/Dockerfile](../../tools/debian-image/Dockerfile) 构建，由容器内的 build-image.sh 核对固定来源并安装工具；脚本不是宿主安装入口，也不是产品制盘 / 运行入口。构建参数、包清单与验收方法统一见[开发环境清单第 2.6 节](environment.md)。后续 xtask 若使用此工具环境，应调用同一配方，不复制包版本与安装逻辑。仓库治理检查只检查这些文件的文本规范，不自动构建镜像或验收工具行为。

| 改动 | 必要相关验证 |
| --- | --- |
| 文档 / 治理 | 本地链接、UTF-8 / LF / 末尾换行、根入口同步、范围 / T / AT 与依赖引用 |
| 领域服务 / 命令 | fmt / check、相关宿主测试、结构化结果与错误、no_std 编译 |
| UI / 输入 | 相同视图宿主复核、真实 QEMU 键盘 / 鼠标 / 焦点 / 状态 |
| 存储 / DMA / 电源 | 目标 guest、边界与失败注入、正常重启、宿主独立读盘 |
| 镜像 / 安装 | 独立副本、危险目标拒绝、读回校验、移除介质与干净 UEFI 启动 |
| 阶段候选 | 对应门槛和 demo；最终候选按全部 AT 及人工清单核对 |

Rust 工程已接入真实 fmt、check、test、clippy 与最终链接检查；只检查文档的工具不能继续被宣称覆盖 Rust。高价值测试验证行为、独立预期、边界和失败，不机械重写实现。低影响文案不要求无关全量测试；只有新变化、失败或未解决风险才扩大复验。

具体命令是否已存在只看实际文件和当前状态。自动化、CI 或脚本未创建时明确未建立，不列出虚假“通过”。文档检查包含未跟踪文件，普通 git diff 无法代表全部文件。

### T01 本地入口

在仓库根使用已安装的项目工具链执行；以下命令不会安装组件或联网解析 crate。nightly 日期由 rust-toolchain.toml 定义，示例对应当前配置；默认工具链与全局环境保持不变。

```sh
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask check
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --host
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask build --profile dev --fresh
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask build --profile release --fresh
```

`check` 分别运行宿主 / 裸机 check、clippy（拒绝警告）和 fmt 检查；`test --host` 运行 xtask、默认无 SDL 的 host-runner 和 kernel lib 的宿主测试目标，覆盖纯引导、内存、PCI、DMA、块接口、GPT、FAT 字节流与名称、ACPI、时钟 / 等待、队列、计量、Surface / 输入 / 共享视图、文件服务与内置命令契约（不运行硬件入口或打开窗口）。`build` 实际链接并检查 ELF；这些入口均不执行 guest。`--fresh` 在本次独占目录构建并关闭增量；不带该参数时复用 target/kernel 编译缓存，但仍检查本次返回的产物。尚未实现的命令返回 2。

首次进入 Cargo 的编译发生在 xtask 启动前，应先按上述方式选择工具链；xtask 随后验证自身编译器身份、项目工具链来源与外部配置，不修改 shell、rustup override 或全局默认。当前宿主进程管理使用 Unix 进程组，目标宿主为 macOS / Linux，Windows 不在 T01 支持范围。系统需有 git、shasum 和 /bin/kill；Python 独立复核及 CI 辅助使用标准库，CI 辅助脚本需 Python 3.11+。

每次实际执行在忽略的 target/xtask-<pid>-<时间戳>-<进程序号>/（既有记录无进程序号，早期 T01 记录为 target/t01-*；进程序号防止相同墙钟时间戳碰撞） 保存 steps.txt 与分步 stdout / stderr。xtask 子进程默认上限 120 秒，可加 `--timeout-secs 1..3600`；超时停止自己的进程组并回收直接子进程，返回 124；前置 / 构建 / 校验失败返回 1，参数错误返回 2。错误输出指向日志；不能将失败目录中的半成品当作成功产物。独立验收材料保存在 evidence/，其路径和候选身份记录于环境清单。

CI 的 install 步骤才允许从官方源安装 TOML 指定的工具链，并按 Cargo.lock 执行 `cargo fetch --locked`；本地不运行它。verify 步骤复用上述四个入口；本地验证结果不能替代 Ubuntu runner 的实际 Actions 结果。

### T02 镜像与串口入口

已授权的首次依赖准备使用 `cargo +nightly-2026-10-13 fetch --locked`，Limine 资产准备使用 `python3 scripts/fetch-boot-assets.py --download`；这些操作联网，日常 xtask 不自动下载、安装或更新。资产存在时脚本只校验精确哈希，冲突文件不覆盖。本次 Cargo 缓存隔离在 work/cargo-home；复用本机验收环境时为所有命令显式加 `CARGO_HOME="$PWD/work/cargo-home"`，无需修改全局配置。

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask image
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask run --serial
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm
```

`image` 在新目录构建 release ELF，调用锁定的既有 Debian 工具镜像产生 1 GiB GPT / FAT32 开发盘，输出真实路径。容器禁用网络、只读根、无新增 capabilities，仅挂本次输入 / 输出普通目录，`--pull=never`；工具镜像缺失即失败。工具脚本只由 xtask 编排，参数来自 manifest，不提供第二套独立制盘入口。

`run --serial` 创建镜像并完成一次无窗口 UEFI 启动、只读 GPT / DATA 探测、T08 真实服务根目录读取、T09 串口控制台初始化、T06 循环进展及 T10 工作台首帧 / 静止验证，串口保存到本次 boot-*/serial.log。`test --vm` 对正常镜像启动 10 次，再使用独立 boot-test 构建校验 4 个 T02、3 个 T03、7 个 T04、7 个 T05、7 个 T08、7 个 T09、8 个 T06、9 个 T07、3 个 T10、4 个 T11 和 8 次 T12 电源启动；共 77 次启动、68 张引导 / 源盘及 37 张数据副本，逻辑容量均为 1 GiB，实际稀疏占用另记。T04 覆盖写入 / flush、同盘重新启动读回、单 GPT 损坏降级、双损坏拒绝、有效副本冲突拒绝、请求超时隔离及缺少 flush 能力拒绝。`test --vm --storage-only` 复用同一存储用例实现，只运行 7 个 T04 用例，生成 8 张引导 / 源盘与 4 张数据副本；日志明确未执行 T02 / T03 独立回归，不能冒充完整套件。完整套件按资源授权运行，普通 CI 不自动执行 VM。

每次复制全新 VARS；固定 TCG、pc-q35-11.1、qemu64、1 vCPU / 512 MiB、VGA、modern VirtIO-blk，无网络，自动套件无 GUI，保留 -no-reboot。普通启动盘只读；双盘存储 / 服务 / 工作台用例由固件从只读 virtio-scsi / scsi-hd 引导，内核仅操作唯一 VirtIO-blk DATA 盘。t04-write、T05 / T08 / T09 写入及同步失败、T10 写入及失败、T11 四例和 T12 除只读例外的场景，均只打开各自专用 DATA 副本的写权限，使用 cache=writeback、aio=threads、无 snapshot；T06 timeout、T10 分页及 T12 只读同步等场景保持 DATA 只读。所有同盘重开用例与 T12 两进程重启链均按各自契约复用原 DATA，不能替换为新盘。所有盘均为任务普通文件，不挂宿主物理盘。固件路径从 QEMU 实体 prefix/share/qemu 派生并核对哈希；实际 VM 环境是 Homebrew macOS，其他安装布局未验证。

单次无窗口 QEMU 限时为 `min(--timeout-secs, 60)` 秒。除 T12 电源成功例外，宿主按模式观察终态、串口稳定或实际图形交互 / 静止证据后停止自己的进程组并回收，再严格校验序列。T06 独立用例等待 T06 READY / FAIL 或更早失败；正常 / T07 路径通过 QMP 等待首帧、指定交互和静止状态，T06 READY 仅是中间证据。T02–T05 独立用例保留原有终态；T08 / T09 独立用例在完成服务 / 终端断言与设备停止后以对应 READY 结束，预期业务失败仍必须有 Failed 操作和原始原因，T02 / T03 READY 只是中间阶段。T12 成功例必须有精确 guest 原因及 code=0 自然退出，失败例由宿主在保留错误证据后停止，详见[电源入口](#t12-正常同步与电源入口)。其他路径提前退出、无终态、非预期错误或事实不符均失败；超时返回 124。普通路径的宿主终止、致命错误后的 cli / hlt 或单纯重新打开同盘，都不代表正常电源验收。没有 UART 或 CPU 无进展时由宿主超时报告。

正常请求需真实 used token 与响应状态成功，flush 单独确认；停止时确认设备 status=0、关闭 bus master 后释放 queue。请求以 HPET 经过时间限制为 2 秒、复位为 100 毫秒，另保留请求 2000000 次、复位 100000 次独立保险；先达到次数上限时报 PollBudgetExhausted，时钟故障另报 Clock，不混称真实超时。配置一致性仍限 64 次。超时用例实际发布请求再隐藏完成观察，保留全部 DMA 与原始缓冲、拒绝释放 / 复用后 halt；不声称制造了设备物理延迟。缺 flush 用例是明确标记的能力拒绝注入；不声称 QEMU 实际未提供该能力。

写入用例在 DATA 尾部 4096 字节合成区域执行对齐与跨扇区写入并 flush；guest 退出后宿主按独立期望比较整盘，确认 ESP / GPT 及所有非目标字节不变，然后重新启动同一盘只读复核。各次构建、工具身份、命令、PID、镜像摘要、串口、VARS 与失败材料留在 target/xtask-*；后台子进程及容器结束即回收，复制证据不复制大盘。T04 不提供 FAT 文件语义、正常电源流程、GUI 或安装。

### T05 FAT 持久化与独立读取入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --fat-only
```

`--fat-only` 复用完整 suite 中同一 T05 编排，生成 8 张引导 / 源盘与 6 张 DATA 副本，执行 7 次启动：写入、同盘只读重开、只读拒绝、文件目录项写回失败、卷 FSInfo 写回失败、最终设备 flush 失败、损坏 FAT 拒绝。单次仍为 60 秒上限；不得与其他 VM 分组同时使用，分组成功不代表整套 `test --vm` 通过。默认 `run --serial` 完成 T04 只读探测后，由 T08 服务只读查询 DATA，再进入 T06 / T07 循环；自动写入验收仅由 boot-test 白名单调用。

DATA 挂载只经 GPT 验证的 Partition；字节流每次最多操作一个扇区内的字节，不增加缓存。首次 IO 错误锁存并拒绝后续设备访问，避免 File / FileSystem 的 Drop 自动重试隐藏错误。每次挂载最多 100000 次流操作（含 seek），用完明确失败，这是防止异常遍历的次数边界，不是实时超时或未来文件服务的长期配额。成功顺序为文件显式 flush、释放目录句柄、卷 flush、显式 unmount、最终设备 flush；只读挂载无脏写入，流 flush 是无写入动作且不会发设备请求。具体名称限制见 [T05 实施契约](../mvp-plan.md#t05-本批实施契约)。

预置材料由 mtools 写入本次副本；guest 写入后，宿主先结束并 wait QEMU，再通过 `tools/boot/check-fat.sh` 调用固定 mtools / dosfstools。磁盘以只读 bind 挂入无网络工具容器，`mdir -b -s` 核对精确名称，`mcopy` 提取到 ASCII 案例文件名，避免宿主路径规范化混淆 `é` 与 `e` + combining acute；比较长度、字节、SHA-256，分区副本仅运行 `fsck.fat -n`。原磁盘不挂载写入、不修复；DATA 外字节与源盘逐块比较。工具容器限制 1 CPU / 256 MiB / 64 PID，结束按唯一名称回收。

文件 / 卷写回失败是流入口的显式单次写失败注入，后续尝试由首错锁存拒绝；最终 flush 失败将该次 VirtIO 请求类型改为 unsupported，核对真实设备负响应及 Flush 阶段。两者不冒充物理设备损坏。失败副本仍独立观察，记录零长文件、未引用簇、脏标记或过期 FSInfo；文件可读不能推翻先前失败，也不能宣称持久化已确认。无正常关机、重启命令或断电事务保证。

### T06 异常、时钟与循环入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --runtime-only
```

实施与失败契约统一见 [T06 实施契约与验证方案](../mvp-plan.md#t06-实施契约与验证方案)。`--runtime-only` 与其他分组互斥，复用全套同一用例函数，执行 1 次正常循环和 8 个独立用例，生成 9 张引导 / 源盘与 1 张只读 timeout DATA 副本。分组不包含 T02–T05 独立回归；T06 原始九类通过证据来自其最终 39 次套件；T07 后正常路径同时验证图形首帧，既有 T06 独立用例不变。

固定平台使用经 ACPI 表与共享 MMIO 映射校验的 64 位 HPET，period 实测 10000000 fs；PIC / PIT IRQ0 仅唤醒主循环，显式关闭并读回本地 APIC enable 位，不支持 x2APIC。IRQ 只更新原子 tick / pending 并 EOI，不分配、打印或调用文件服务。64 项 FIFO 满时拒绝新事件并计数，主循环每轮最多处理 16 项；内存统计分别报告 Usable 帧与堆范围，未知部分不计为空闲，堆不能重复加入帧分配总数。

早期 IDT 保护启动路径；最终 GDT / TSS / IDT 为 double fault 配置静态 16 KiB IST 和 4 KiB guard。致命异常、panic 与分配失败使用最多 2048 字节、每字节最多 100000 次发送轮询的紧急串口，禁止堆分配、普通锁与析构展开。page fault / double fault / OOM 通过真实机制注入；普通非法名称拒绝后仍处理合法事件。时钟停止、缺失、倒退与不一致不静默 fallback；已提交请求失败原地隔离 DMA，不退出被设备引用的栈帧。

实际结果、首次 IRQ 失败修复、独立 ELF / 磁盘 / 坏日志对照及资源统计见[环境清单第 2.17 节](environment.md#217-t06-异常时钟与事件循环验收)。仅启用已批准的 x86_64 0.15.5 中断 ABI 特性，无新增包、下载、工具链或 lockfile 变化。后续新依赖或新增资源仍按任务授权，不把本轮额度延伸到 T07。

### T07 图形与输入入口

T07 默认启动在只读存储探测后建立 framebuffer 后台页、PS/2 Set 2 键盘与三字节鼠标，进入同一事件循环。T07 输入 / 显示校验页由专用 `t07-input-check` 保留完整基础回归；默认图形界面已切换到 T10 共享工作台。内建 ASCII 字体、焦点、容量拒绝和 InputReset 有可见状态；初始化失败保留串口阶段 / 端口原因，不能出现 T07 READY。

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask ui --host
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask ui --host --gui
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --graphics-only
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --graphics-cases t07-input-overflow,t07-redraw
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask run --gui
```

`ui --host` 无窗口生成同视图 PPM 与状态；`--gui` 显式链接固定本机 SDL 兼容层并打开真实窗口，macOS 应用包仅在本次输出目录生成。Esc 清空输入，关闭窗口退出；失焦清除解码器与按键 / 按钮状态。GUI 单次限 600 秒，首次 host 构建单次限 600 秒；总资源仍遵循 current 预算。SDL 路径仅覆盖已确认 macOS 宿主；默认无 SDL 的 check / test 可进入 CI，不把缺失 GUI 环境当通过。

`--graphics-only` 共 10 次启动：默认 T10 工作台交互、T07 专用完整输入校验和 8 个独立注入 / 边界用例；`--graphics-cases` 只接受同一列表中的逗号分隔名称，拒绝未知 / 重复项，不创建额外正常盘，不代表全套完成。`test --vm` 包含 10 次正常、既有 29 个独立 T02–T06 用例、9 个 T07 用例、7 个 T08 用例、7 个 T09 用例、3 个 T10 用例、4 个 T11 用例和 T12 七类电源场景的 8 次启动，共 77 次。QMP 使用任务私有 Unix socket、结构化 JSON、3 秒回复 / 64 KiB 上限；每例仍限 60 秒。实际接收计数决定注入进度，不能把发送数当硬件接收数。

真实 PPM 检查角点、色序、字形行、裁剪与光标，并与同状态宿主像素比较；被破坏的实际截图必须拒绝。初始化 READY 不能替代交互终态，静止后的两次观测应保持帧代次不变。Cocoa 入口关闭全局抓取，等待真实窗口键盘释放、左键释放和激活后保存证据；宿主结束 QEMU 仍不算 guest 正常关机。生成目录保留串口、状态、QMP 往返、PPM、像素报告、盘和 VARS；精确候选与运行范围见 current。

### T08 类型化文件服务入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --service-only
```

`--service-only` 与其他 VM 分组互斥，复用完整套件中的七例：创建、同盘只读重开、只读拒绝、FileSync / VolumeSync / DeviceFlush 失败及损坏 FAT。生成 8 张引导 / 源盘和 6 张数据副本；单次 VM 限 60 秒。只验证此组不代表完整 77 次套件通过。实际结果和范围见[环境清单第 2.20 节](environment.md#220-t08-共享文件服务验收)。

生产入口为 `kernel::services::FileService`，真实后端为 `FatBackend`；`Request` / `Reply` 提供 list、分页 read 与 create-text。操作记录、错误、部分副作用、变更事件、启动 / 卷 / 代次绑定的引用均由同一服务生成。路径、上限和失败契约见 [T08](../mvp-plan.md#t08-实施契约与验证方案)。宿主替身仅在 cfg(test) 下存在。100000 次 Stream 调用预算可能先于满卷遍历结束，此时明确报 ResourceLimit / Budget 并离线，不能据此声称 NoSpace。

每次启动由 xtask 从 OS 随机源生成 16 字节非零身份，以 QEMU `fw_cfg` 只读项提供并保留 `boot-id.bin`；当前仅支持固定 q35 平台，身份缺失或非法即失败。默认 T04 探测后将仍活动的唯一 Driver 交给服务，输出 `LIVE owner=file-service`；独立存储用例仍执行设备停止 / DMA 释放核对。服务先读 DATA 根目录并保留实例，随后进入原有图形循环；T09 串口控制台共用该服务；图形终端和文件列表由 T10 复用该服务。

真实写入只有 file flush、volume flush、unmount 和最终设备 flush 全部成功才发布成功。服务失败注入在适配器入口执行一次写 / flush 拒绝，日志标注 `adapter-once`；这与 T05 最终 flush 的真实设备 unsupported 响应不同，不混称物理设备故障。首错后封锁后续 IO，记录创建、已确认负载字节和可能修改，并让服务离线；不删除半成品。mtools / dosfstools 与独立 Python GPT / FAT 读取在 guest 退出后检查内容、只读不变和 DATA 外不变；失败盘中的可读内容不代表持久化已确认。

### T09 内置命令与行输入入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --shell-only
```

`--shell-only` 与其他分组互斥，复用全套七例：外部串口编辑 / 命令写入、同盘只读重开、只读拒绝、FileSync / VolumeSync / DeviceFlush 失败及损坏 FAT。生成 8 张引导 / 源盘、6 张专用 DATA 副本；单 VM 60 秒。宿主逐字发送并等待实际接收确认，以防 UART 输入队列溢出；未确认不能当成已输入。文件只由 guest 的共享服务创建，宿主按独立预期读回。成功分组不代表完整 77 次 suite 或 M1 通过。

T09 建立六个基础内置命令，T12 在同一 `kernel::shell::parser::COMMANDS` 增加 sync / reboot / shutdown；当前共九个，help 和所有入口共用该表。Command / Reply / Output、1024 字节行编辑器与 UART 输入解码保持共享；具体引号、转义、游标、历史、分页和 ID 规则见 [T09 实施契约](../mvp-plan.md#t09-实施契约与验证方案)，电源扩展见 [T12 实施契约](../mvp-plan.md#t12-实施契约与验证方案)。`write` 只创建，`cat` 每次 4096 字节并报告 offset / eof，`ls --next` 使用服务校验过的游标。`operations` 与完整 ID 的 `explain` 查询同一记录，解析失败不伪造文件操作；记录缺失和事件淘汰明确返回。

默认启动在 T08 服务 Ready 后建立控制台，每轮最多接收 32 字节；COM1 读写保持单 BSP 所有权，UART 库保留的 LSR 接收错误会使当前行拒绝。正常行可用左右 / Home / End / Delete、上下历史、退格、Enter；Esc 等待最多 100 ms 区分方向序列后取消，Ctrl-C 立即取消。UTF-8 / 控制序列 / 容量损坏后只接受换行或 Ctrl-C 恢复，不执行残余文本。当前不提供 ANSI 终端兼容性、PTY 或外部程序。

默认 / T07 / T09 VM 的串口改用权限 0700 任务目录内的 Unix 字符 socket，路径记为 `console_private_socket`，所有输出仍保存到 serial.log。`RR:CONSOLE:REPLY` 为转义 JSON 展示投影，保留数值、布尔、空值、文件引用、原始 bytes hex / 可解码文本，以及完整操作错误与副作用；不能把未知编码解释成文件损坏。输入编辑、确认和拒绝使用独立 CONSOLE 帧，文件内容中的换行不会伪造验收记录。

既有 `run --serial` 仍在默认服务 / 图形首帧和静止检查完成后自动结束，`--shell-only` 则由宿主完成外部命令后结束；没有新增常驻终端窗口。图形终端、输出滚动与双面板的接入见 T10。真实结果、宿主 / guest 逐字段等价性、负对照和资源见[环境清单第 2.21 节](environment.md#221-t09-最小终端与操作记录验收)。

### T10 双面板工作台入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask run --serial
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --workbench-only
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask ui --host --disk <专用镜像副本>
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask ui --host --disk <专用镜像副本> --gui
```

`--workbench-only` 复用完整套件同一三例：图形创建 / 冲突 / 原始字节读取、FileSync 单次失败 / 部分修改 / 离线、201 项真实目录跨七页。生成 4 张引导 / 源盘和 3 张 DATA 副本，单 VM 60 秒；可写盘仅为本批专用 DATA 副本。默认盘仍只读，写入被真实拒绝。`check-fat.sh prepare` 按同一合成材料清单逐项预置，重复材料标签由工具分配唯一输出名；guest 退出后按路径、字节、只读摘要及 DATA 外不变独立复核。不代表完整 77 次套件或 M1 通过。

共享 `kernel::workbench::View` 提供固定 `/data`、显式刷新 / 下一页、本页行选择 / 滚动、图形行编辑、最近结果与操作详情；点击或 Tab / Enter 激活同一动作。Panels 在终端铺满、文件区铺满和双面板间切换，小屏保持单面板。终端复用 Shell / Editor，文件区直接发 FileService 请求；T10 的显式快照行为由 T11 自动联动扩展；当前列表按变更重新查询，终端历史仍保留原快照。详情 Drawer 可关闭、独立滚动；解析错误没有伪造操作 ID，过期游标和记录淘汰保留真实错误。

行输入最多 1024 字节，溢出整行拒绝，Enter / Esc 清除；失焦 / InputReset 取消未提交草稿并重置按键和按钮。结果保留最多 16 条且保守计量不超过 256 KiB，展示最多 512 行 / 64 KiB；显示截断不改原始类型与字节。非 ASCII 使用 `?` 并说明字形限制，Bytes 同时保留 hex，不能推断文件损坏。

`ui --host --disk` 仅读取明确给出的普通 GPT / FAT 镜像，调用同一 FatBackend；不列宿主真实目录、不写源镜像。无 `--disk` 仍为 T07 校验页。实际 guest 输入与服务执行步骤以 `RR:T10:STEP` 记录，宿主在相同只读镜像及 boot_id 上回放并逐像素比较；可写案例只声明独立像素 / 类型结果 / 磁盘验证，不宣称做过可写宿主像素回放。`RR:T10:REPLY` / `DETAIL` 使用已有类型投影，独立期望校验成功、错误阶段和同操作 ID。

macOS 窗口使用任务目录内的唯一 bundle 身份；QEMU 副本与原件摘要相同，`-L` 显式指向锁定固件目录，不修改 Homebrew。SDL 原生鼠标绝对坐标转为共享输入事件；关闭窗口退出，超时回收。GUI / VM 按当前批额度执行；结果及 Cocoa 修饰键补验限制见[环境清单第 2.22 节](environment.md#222-t10-双面板工作台验收)。

### T11 自动刷新与原操作联动入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --link-only
```

`--link-only` 与其他分组互斥，复用完整套件同一四例：图形创建 / 冲突 / 原历史、201 项目录中新文件的自动跨页查找、FileSync 失败后的自动列表错误、串口创建到图形列表 / 原操作详情的联动。生成 5 张引导 / 源盘及 4 张 DATA 副本，均为任务普通文件；单 VM 60 秒，结束后独立 FAT / DATA 外检查。不发送 Refresh 动作冒充自动联动，不代表完整 77 次套件或 AT05 / M1 通过。

`workbench::observe_changes` 只检查有界服务变更历史，按启动 / 卷 / 代次订阅，直接子项事件才触发固定 `/data` 重查；无关子目录和无修改拒绝不查询。目标跨页时每轮只读一页，最多 128 页；命令和详情优先，草稿不被刷新清空。代次变化合并并最多重启 4 次，超过显式停止；事件缺口重查第一页并显示原因，不猜测已丢目标。原列表读取错误保留真实阶段和原因，不能再用一次 Offline 查询覆盖它。

文件区显示刷新状态与原变更操作来源，Enter / 点击文件区打开该原操作；列表查询自身仍有独立 ID。终端历史保持类型与字节快照，详情按所选原 ID 查询，淘汰 / 异启动显式报错。`RR:T11:PAGE` / `ITEM` 记录实际页、来源和 UTF-8 名称 hex，配合原 `FRAME` / `REPLY` / `DETAIL` 及独立磁盘预期核验；不能把新的列表操作当成原写入。

T11 外部命令与 T09 共用同一有界串口输送入口，逐字发送、等待实际接收、消费 socket 输出防止回压；输入完成和 T09 场景终态由明确类型区分。宿主与 guest 仍用相同 View；只读路径继续按实际步骤回放逐像素比较，可写联动案例仅声明真实帧、类型结果与独立磁盘事实。实际结果和未覆盖项见[环境清单第 2.23 节](environment.md#223-t11-自动刷新与操作联动验收)。

### T12 正常同步与电源入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --power-only
```

`--power-cases CASE,...` 只从同一七例白名单选择，拒绝未知 / 重复名称并复用相同编排；结果明确标为子集。`--power-only` 与其他分组互斥，复用完整套件的七类场景、8 次 QEMU 进程：显式 sync 后继续写入、正常 shutdown、guest reset 后同盘读回并 shutdown、VolumeSync / DeviceFlush 失败、平台拒绝及只读同步。生成 9 张引导 / 源盘和 7 张 DATA 副本，均为 1 GiB 稀疏普通文件，单进程最多 60 秒。失败例由宿主停止，成功例必须由 guest 请求自然退出；受控 reboot 的两个进程分别记账。不据此宣称完整 77 次、完整 AT14、安装或 M1 通过。

Shell 命令表是九命令的唯一来源，`sync` / `reboot` / `shutdown` 不接受参数；图形和串口经同一 FileService 请求。单 BSP 独占服务先停写 / Drain，再挂载并同步卷、显式 unmount、取得真实设备 flush 响应；没有跨请求文件句柄或异步写缓存。只读 sync 也真实 flush 设备，不写扇区。成功 sync 恢复写入，电源准备后继续停写；后续修改明确 ShuttingDown，同步本身不发布文件 ChangeEvent。操作记录在电源状态存续期间保留，诊断请求仍按有限容量淘汰其他记录。

电源准备输出为 `power-prepared` / `power_completed=false`，不表示关机已经成功。共享工作台显示 live power 状态及原始错误，终端历史保持准备时快照；explain 查询同一原操作的后续阶段。平台确认固定 FADT / ICH9 端口后，经 Quiesce 确认设备 reset=0、关闭 bus master、回收 DMA，再记录 Requested 并执行一次寄存器请求；寄存器请求后仍运行超过 500 ms 时转为 PowerNotTaken，保留诊断界面。同步、设备停止或平台失败阻断正常电源，没有自动重试或强制关机入口。

`RR:T12:PREPARED` / `STOP` / `REQUEST` 分别记录真实同步边界、设备安全停止和平台请求；独立校验要求顺序及操作 ID 一致。QMP 必须提供 guest=true 且 reason 精确匹配的 SHUTDOWN 事件，进程须自然退出为 0；缺失、重复、host 请求或错误原因均失败。保留 `-no-reboot` 将 guest reset 变为可观察退出，然后编排自动重新打开同一 DATA 副本，以 OS 随机源发放新 boot_id；这是两个进程的受控重启链，不是同进程连续重启。新 boot_id 不依赖时钟或原镜像固定身份。

`--power-only` / `--power-cases` 验证专用电源路径，T13 的 [`demo`](#t13-m1-自动演示入口) 复用自然退出识别和同盘重开。`run --serial` 在首帧 / 静止检查后由宿主结束，`run --gui` 在短交互 / 静止检查后由宿主结束。完整 Cocoa 演示使用 [`demo --gui`](#t13-cocoa-人工演示入口与清单)。宿主 `ui --host --disk` 只共用只读服务与视图。

失败注入仅限 boot-test：卷 / flush 在既有同步适配器明确单次失败，平台例明确拒绝能力，不冒充物理故障。所有盘在 guest 退出后以独立 mtools / dosfstools 只读核对精确路径 / 字节 / 结构及 DATA 外不变；可写场景不宣称宿主像素回放。真实验收结果、候选和资源以 current 指向的报告为准。

### T13 M1 自动演示入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask demo
```

按 [T13 首轮契约](../mvp-plan.md#t13-首轮实施契约)运行两次无窗口 QEMU，使用普通 release 内核、一个只读引导盘和同一份独立 DATA 副本。首启可写，实际键鼠走工作台，完成创建、自动列表、原操作详情、冲突与正常关机；中间独立读盘；次启只读，校验原内容和旧操作引用被拒后正常关机。QMP 帧、串口、独立 FAT 材料、盘、进程退出状态和工具输出保存在当次 `target/xtask-*`。每次启动上限为 `min(--timeout-secs, 60)` 秒，其他子进程沿用 `--timeout-secs`。入口成功返回 0，行为或数据错误返回 1，参数错误返回 2，进程超时返回 124。自动演示与人工 GUI 和完整 M1 验收分别记录。

同步失败对照沿用 `test --vm --power-cases t12-volume-fail`，使用单独 DATA 和 boot-test 注入。运行前确认本批 VM、专用盘、资源与清理额度。

### T13 AT06 命令等价入口

```bash
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --host --shell-equivalence
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --shell-equivalence --timeout-secs 60
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --workbench-equivalence --timeout-secs 60
```

按 [AT06 命令等价契约](../mvp-plan.md#t13-at06-命令等价复验)运行。宿主入口保留输入和宿主轨迹；VM 入口先验宿主，再跑一次无窗口 QEMU，使用 boot-test、只读引导盘和一张可写 DATA 副本。串口逐字节回执、同序的结构化回复、整行丢弃、写入副作用和设备停止都须通过。guest 退出后核对独立 FAT 读盘和 DATA 外字节。原始轨迹、串口输入、负对照和盘保存在当次 `target/xtask-*`。此入口单独确认运行额度，覆盖宿主与串口 guest；图形入口证据与全套 VM 结果分别记录。

`--workbench-equivalence` 单独启动一次无窗口 QEMU，用 QMP 键盘事件驱动 guest 的图形终端。从 guest 解码记录重建相同的命令字节，比较宿主回复并核对图形端额外的三次只读查询。保存超限、Enter 清空、下一条命令恢复三帧及其输入记录，退出后使用同一套独立读盘检查。额度单独确认，运行无需实体键鼠。

### T13 Cocoa 人工演示入口与清单

窗口、专用盘和运行额度获准后，在 macOS 固定验收宿主执行下列命令。使用原有 QEMU 与固件，任务目录为每次启动创建独立 Cocoa bundle 和 UEFI VARS。`--timeout-secs` 默认为 120 秒，窗口每次最多 600 秒，示例将每次窗口限为 300 秒。超时返回 124 并回收任务进程。关闭窗口或退出 QEMU 会按实际退出原因判定。

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask demo --gui --timeout-secs 300
```

使用实体键盘与鼠标。guest 键盘为 US 布局，关闭 Caps Lock，双引号用 Shift 加单引号键，冒号用 Shift 加分号键。按控制台提示操作，每个检查点等待 `Recorded` 后再继续，取帧期间保持指针静止并留在工作区。提交了错误命令、跳过检查点或误关窗口时，记录实际现象并保留当次目录。QMP 仅采集窗口帧和退出事件。

| 阶段 | 操作与人工判据 |
| --- | --- |
| 首启与创建 | 确认 1024 × 768 双面板、文字可读且输入区有焦点。键入 `write /data/hello.txt "hello RadishRoot"` 并按 Enter。文件区自动出现并选中新文件。 |
| 创建详情与焦点 | 点击左侧选中的文件行。核对详情 ID 与写入结果一致，等待取帧。按 Tab 一次，确认详情的 `Up` 获得可见焦点。取帧后按 Shift+Tab 回到 `Close`，释放 Shift，再按 Enter，确认详情关闭、焦点回到输入区。 |
| 冲突与关机 | 输入 `write /data/hello.txt again`。核对 `AlreadyExists`，点击终端的 `Details`，确认失败阶段和原拒绝 ID。取帧后按 Esc，输入 `shutdown`。等待 guest 退出及宿主独立读盘，校验通过后编排会打开第二个窗口。 |
| 同盘再启动 | 在第二个窗口确认输入区有焦点，必要时按 Esc 回到输入区。输入 `ls /data`，等待取帧。再输入 `cat /data/hello.txt`，核对 `hello RadishRoot`。取帧后用实体 Shift 键输入控制台给出的 `explain <旧操作 ID>`，确认旧引用被拒。取帧后输入 `shutdown`，等待编排输出结果。 |

每次执行记录候选、操作者是否使用实体键鼠、实际命令与观察、两个启动 ID、操作 ID、检查点帧和独立读盘报告。原始串口、QMP、`.steps` 输入记录和 `workbench-screen-report.json` 随盘保存在当次 `target/xtask-*`。机器断言在首窗复核鼠标释放、详情焦点与键盘关闭，在两窗复核键盘释放、对应的 Shift 组合、正常退出和真实盘字节。人工检查记录原生窗口中的可读性、焦点表现和输入来源。AT06 等价、完整 AT07 及资源测量分别登记。

## 5. 视觉与数据材料

采用 RadishX family-ui v26.7.3 的 Workbench 方向，项目映射以 MVP 规划为准。参考规则而非搬运外部截图、品牌或整包 UI；具体设计资产若使用 .pen，只通过支持的 Pencil 工具读写。

UI 状态随着真实行为扩展，优先可读性、焦点、错误、长内容与资源上限。产品语言与文档语言分别处理，英文界面不允许损坏 UTF-8 路径。

颜色、间距和状态样式优先由已有语义 token 或明确的项目映射表达，不让不同页面随意增加硬编码值。复用组件与服务边界，分别核对视觉可读性及真实键盘 / 指针 / 滚动行为；不把设计图当作已实现的交互证据。

测试只用合成内容与任务专用磁盘；大镜像、敏感日志和缓存不入 Git。生成物记录生成入口、输入与版本；关键数据使用独立工具或期望复核。

## 6. 文档与交付

README 稳定定位；AGENTS / CLAUDE 启动级规则；产品 / MVP 专题维护正式契约；current 只记当前能力、阻塞、下一步；月度 devlog 保存过程；ADR 保存关键决定。具体职责见[文档导航](../README.md)。

提交遵循[仓库治理](../governance/repository-governance.md)，授权与清理遵循[协作规则](../governance/agent-collaboration.md)。交付写最终结果、实际证据、未验证项、风险和仓库状态；不把静态检查或宿主演示扩大成产品完成。
