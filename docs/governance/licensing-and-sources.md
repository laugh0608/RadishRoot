# 许可与来源策略

## 原创内容

RadishRoot 的原创许可证尚未选择。建立文档、治理工具与后续代码不代表可以复制兄弟项目的 LICENSE，不宣称已采用开源或源码可见授权，也不据此开放公共再分发。首次对外提供源码、镜像或安装介质前，明确原创授权范围、贡献条款、第三方义务并加入正式文件。

这不阻止按当前授权进行本地规划与后续开发；发行相关验收按实际用途核对，不用泛泛的“已审查依赖”替代产物清单。

## 第三方材料

引入时记录项目 / 资产、官方来源、精确版本或 commit、许可证、功能开关、修改情况、必要声明和拟分发位置。Rust 库、引导程序、OVMF、Linux Live 内容、字体、图标分别核对；源代码许可不能自动代表二进制介质可按相同方式分发。

版本未冻结前只保存候选与验证计划。下载、安装或新增资产按实际授权；不能将公开可访问视为可自由再分发。正式引入后建立 third-party 清单，禁止现在填入未使用组件的虚假合规结论。

## 家族参考与本文来源

本次只读参考本机 `~/Code/` 下 RadishLedger、RadishFlow、RadishLex、RadishMemory、RadishAxiom、RadishNexus 和 RadishX 的 README、协作 / 开发 / 状态文档与部分工程入口，抽取共通习惯并在本项目重新表述。兄弟文档不是本项目隐式配置，不跨目录写回，也不迁入完整实现。

视觉参考为 RadishX `docs/design/family-ui/README.md` 的 v26.7.3 Workbench 原则；本地映射由本项目维护，升级需主动复核。外部参考截图、字体或图片不得直接当交付素材。

`docs/assets/ui-layout.png` 来自本对话为原始项目规划制作的布局示意，仅作信息结构参考，不是正式产品界面截图。本次保留原图，不将其配色当作已冻结的 family-ui 映射。

组件官方参考链接集中在 [MVP 规划](../mvp-plan.md)第 19 节。文档重排没有证明任何依赖组合已经可编译或运行。

2026-10-14 治理补全只读参考 Radish、RadishMind、RadishCatalyst、RadishFlow、RadishAxiom；相关路径、HEAD 与采用 / 排除理由见 [ADR 0002](../adr/0002-repository-foundation.md)。按原则重新组织本项目文件，没有整包迁入兄弟代码、测试、素材或 LICENSE；HEAD 仅标记参考时点，工作树内容不是依赖锁。

CI 使用 GitHub 官方 actions/checkout v4.2.2，固定 commit `11bd71901bbe5b1630ceea73d27597364c9af683`，上游为 MIT：[release](https://github.com/actions/checkout/releases/tag/v4.2.2)、[LICENSE](https://github.com/actions/checkout/blob/v4.2.2/LICENSE)。2026-10-14 已由 GitHub hosted runner 获取并执行，真实 Actions 证据见[月度日志](../devlogs/2026-10.md)；本地未执行该 Action。它仅用于 CI，不进入系统镜像。宿主已有 Python 标准库用于治理检查，无新增 pip 依赖；它不是 guest 运行依赖。任何后续打包分发须按实际清单重新核对。

## 本地开发工具清单

2026-10-14 在用户授权下构建本地 Debian 工具镜像，实际身份与包体 / 包清单证据见[开发环境第 2.6 节](../development/environment.md)。用途限宿主侧 ARM64 GPT/FAT 开发工具，不是 RadishRoot guest 组件；没有源码补丁、自行编译或改变 Debian 包的编译开关，包按精确版本原样安装。没有将 `.deb`、容器层或工具二进制提交仓库，也没有推送 / 分发镜像。

| 已使用材料 | 固定来源 / 版本 | 本次读取的许可声明与保留位置 |
| --- | --- | --- |
| Debian 基础系统（78 包） | `debian@sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850`；[原始版本清单](../../tools/debian-image/base-packages.tsv)来自官方构建来源 `cf1f4a45447842b45e9952e0f018ae734a7341c7` | 基础系统没有统一覆盖全部包的许可证；沿用包内 `/usr/share/doc/*/copyright` 和 `/usr/share/common-licenses`，本轮未逐项重审基础 78 包许可 |
| [gdisk / gptfdisk](https://sourceforge.net/projects/gptfdisk/) | Debian ARM64 `1.0.10-2+b1` | 该版本 Debian copyright 声明 LGPL-2.0+；保留 `/usr/share/doc/gdisk/copyright` |
| [dosfstools](https://github.com/dosfstools/dosfstools) | Debian ARM64 `4.2-1.2` | GPL-3+；src/blkdev 部分声明 public-domain；保留 `/usr/share/doc/dosfstools/copyright` |
| [GNU mtools](https://ftp.gnu.org/gnu/mtools/) | Debian ARM64 `4.0.48-1` | copyright 声明 GPL-3，手册部分 GFDL-1.1；保留 `/usr/share/doc/mtools/copyright` |
| [ncurses / libncursesw6](https://invisible-mirror.net/archives/ncurses/) | Debian ARM64 `6.5+20250216-2` | MIT/X11，列明部分 X11 / BSD-3-clause；保留 `/usr/share/doc/libncursesw6/copyright` |
| [popt / libpopt0](https://ftp.osuosl.org/pub/rpm/popt/releases/popt-1.x/) | Debian ARM64 `1.19+dfsg-2` | 主体 expat，翻译部分 public-domain，Debian 包装 GPL-2+；保留 `/usr/share/doc/libpopt0/copyright` |

五包来自 `20261005T000000Z` Debian snapshot，包体 SHA-256 以 [packages.tsv](../../tools/debian-image/packages.tsv) 为单一构建输入；镜像内完整 83 包清单为 `/usr/local/share/radishroot-debian-tools/packages.tsv`。上述许可标识按实际包内 copyright 记录，保留原声明与署名，不将摘要当作全部文件或未来发行的许可结论。若将来对外分发，再按实际产物处理相应源码、许可文本和归属义务；本地工具构建不替代 RadishRoot 原创许可证选择。

### 本机 QEMU 与随包固件

2026-10-19 用户通过正常 Homebrew 安装 QEMU 11.1.2；本轮只读核对实际收据、SBOM、运行与固件文件，结果见[开发环境第 2.8 节](../development/environment.md)。用途限宿主侧仿真工具；未向仓库复制 QEMU / 固件二进制，未启动 guest，未打包或分发这些材料，也未自行修改 / 编译。

| 已安装材料 | 来源 / 身份 | 实际读取的许可与保留位置 |
| --- | --- | --- |
| QEMU 11.1.2，revision 0 / bottle rebuild 0 | 官方 homebrew/core 的 arm64_golden_gate bottle；SBOM 根制品 SHA 与环境清单候选相符，收据声明 poured_from_bottle。上游 [QEMU v11.1.2](https://github.com/qemu/qemu/tree/v11.1.2) | keg 内 LICENSE 说明模拟器整体为 GPL version 2、源码文件另有兼容声明，且固件具有单独许可；保留 LICENSE、COPYING、COPYING.LIB 和 sbom.spdx.json，不以单一 formula 标识覆盖所有文件 |
| QEMU 随包 EDK II x86_64 code / VARS | 以安装包 descriptor、文件尺寸与环境清单的 SHA-256 识别；上游 [pc-bios README](https://github.com/qemu/qemu/blob/v11.1.2/pc-bios/README)为来源说明，不能代替已装字节身份 | `share/qemu/edk2-licenses.txt` 中 EDK II / OvmfPkg 声明 BSD-2-Clause-Patent，另含 MIT、历史许可、SoftFloat、OpenSSL / SSLeay 等声明；保留完整文本，不将汇总内容全部推定为这两份固件的实际链接组成 |

其余 29 项 Homebrew 运行依赖的实际版本与包收据已记录在环境证据中，本轮没有逐项完成分发许可审查。许可证摘要仅记录本机工具来源；未来若分发 QEMU、固件或其他依赖，须按实际制品重新处理许可、源码与归属义务。宿主 OpenSSL 4 与固件内嵌密码库属于不同材料，不能用宿主版本推断固件的库版本或许可。

### 本机 Rust nightly 工具链

2026-10-19 用户安装 `nightly-2026-10-13-aarch64-apple-darwin`，本轮只核对工具与文件，未编译、修改或分发 Rust 材料。来源为此前核对的 [Rust 官方日期清单](https://static.rust-lang.org/dist/2026-10-13/channel-rust-nightly.toml)；已装七项组件的版本、归档 URL 和摘要与该清单一致，实际 rustc commit 为 `e9bbadf6824e65b3583c2dda3a0926ec76df9143`，链接器为 LLD 23.1.3。详细身份与核验边界见[开发环境第 2.9 节](../development/environment.md)。

随包 `share/doc/rust/README.md` 声明 Rust 主体使用 MIT / Apache-2.0，部分材料适用其他许可；完整归属由 `COPYRIGHT.html`、`COPYRIGHT-library.html` 与 `licenses/` 文本记录，已在忽略证据目录中保留。该摘要不将所有组件、LLVM 或未来静态链接进内核的库归为同一许可。2026-10-19 仅登记本机工具来源。2026-10-25 的 T01 已使用该工具链与预编译 core 生成本地静态 ELF；两个本地包没有 crates.io / Git 依赖，未复制第三方源代码或分发产物。实际发行时仍须按最终组成补齐许可及归属，不把工具来源记录或本地链接成功当作完整分发审查。

## T02 已引入的引导组件

2026-10-25 按用户明确授权引入下表，完成真实构建、UEFI 启动及串口负向验证。源码和二进制均未做上游补丁；本地适配代码独立实现。Cargo.lock 保存 registry 包摘要，[引导 manifest](../../tools/boot/manifest.tsv)保存 Limine 归档、EFI、许可文件及已有工具 / 固件摘要；这些身份不由随手下载的 latest 替代。

| 材料 | 官方来源 / 精确身份 | 使用方式与声明 |
| --- | --- | --- |
| Limine 引导器 | [v12.9.4](https://github.com/Limine-Bootloader/Limine/releases/tag/v12.9.4)，limine-binary.tar.xz；对应协议 commit `3a0526b700e356f0eac1b71a77697b3fd1c707a3`，base revision 6 | 使用原始 BOOTX64.EFI；主体 BSD-2-Clause，完整 LICENSE 与 3RDPARTY.md 从同一归档校验后随 ESP `/licenses/` 保存。3RDPARTY.md 包含其他实际声明，不以主体许可覆盖它们 |
| limine Rust 绑定 | [limine-boot-rust](https://github.com/robotman2412/limine-boot-rust)，[crate 0.6.5](https://crates.io/crates/limine/0.6.5) | MIT OR Apache-2.0；选择 MIT，default-features=false；仅协议请求 / 响应，无 std 或 allocator；[原 MIT 文本](../../tools/boot/licenses/limine-0.6.5.txt) |
| UART 驱动 | [rust-osdev/uart_16550](https://github.com/rust-osdev/uart_16550)，[crate 0.9.0](https://crates.io/crates/uart_16550/0.9.0) | MIT OR Apache-2.0；选择 MIT，default-features=false；使用 PIO 与有界非阻塞发送，不使用 TTY 无界等待；[原 MIT 文本](../../tools/boot/licenses/uart_16550-0.9.0.txt) |
| bitflags | [bitflags/bitflags](https://github.com/bitflags/bitflags)，[crate 2.13.2](https://crates.io/crates/bitflags/2.13.2) | UART 必要传递依赖；MIT OR Apache-2.0，选择 MIT；[原 MIT 文本](../../tools/boot/licenses/bitflags-2.13.2.txt) |

三份 crate MIT 文本来自已按 Cargo.lock 校验的包内 LICENSE-MIT，仅为原 limine 文本补齐缺失的末尾 LF，其余字节不变；作为署名材料保存并放入 ESP `/licenses/`；没有整包源码进入仓库。Limine 原始文件保留在忽略的 work/t02-assets，归档内容使用固定成员白名单与尺寸 / 哈希校验，不使用不受约束的 tar 解包。EFI 及 crate 制品未对外分发。

既有 QEMU / EDK II 固件与 Debian 容器工具仍仅为本地验证工具；固件不进入开发盘，完整声明保留在既有安装环境。Rust core / compiler_builtins 等工具链组成仍按前节来源记录；本批本地验证不替代未来发行的完整材料清单、原创许可选择及分发审查。

## T03 已引入的内存与设备组件

2026-10-25 按用户授权增加 19 个 registry 包，保留 T02 既有包版本；Cargo.lock 现在包含 2 个本地包及 22 个外部包。上游源码未修改；本地仅实现平台适配。直接依赖固定 x86_64=0.15.5（仅 instructions）、linked_list_allocator=0.10.6（无 use_spin）、virtio-drivers=0.13.0（无默认 feature），限定 target_os=none。宏 / 构建依赖在宿主执行，不作为 guest 的 std 运行库；具体传递 features 与 checksum 由本次 metadata / tree / lockfile 共同记录。

| 包 / 精确版本 | 官方仓库 | 上游许可与本次保留 |
| --- | --- | --- |
| bit_field 0.10.3 | [bit_field](https://github.com/phil-opp/rust-bit-field) | Apache-2.0/MIT；选择 MIT；[完整文本](../../tools/boot/licenses/bit_field-0.10.3.txt) |
| const_fn 0.4.12 | [const_fn](https://github.com/taiki-e/const_fn) | Apache-2.0 OR MIT；选择 MIT；[完整文本](../../tools/boot/licenses/const_fn-0.4.12.txt) |
| enumn 0.1.14 | [enumn](https://github.com/dtolnay/enumn) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/enumn-0.1.14.txt) |
| linked_list_allocator 0.10.6 | [linked_list_allocator](https://github.com/phil-opp/linked-list-allocator) | Apache-2.0/MIT；选择 MIT；[完整文本](../../tools/boot/licenses/linked_list_allocator-0.10.6.txt) |
| log 0.4.34 | [log](https://github.com/rust-lang/log) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/log-0.4.34.txt) |
| proc-macro2 1.0.107 | [proc-macro2](https://github.com/dtolnay/proc-macro2) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/proc-macro2-1.0.107.txt) |
| quote 1.0.47 | [quote](https://github.com/dtolnay/quote) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/quote-1.0.47.txt) |
| rustversion 1.0.23 | [rustversion](https://github.com/dtolnay/rustversion) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/rustversion-1.0.23.txt) |
| safe-mmio 0.3.2 | [safe-mmio](https://github.com/google/safe-mmio) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/safe-mmio-0.3.2.txt) |
| syn 2.0.119 | [syn](https://github.com/dtolnay/syn) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/syn-2.0.119.txt) |
| syn 3.0.9 | [syn](https://github.com/dtolnay/syn) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/syn-3.0.9.txt) |
| thiserror 2.0.21 | [thiserror](https://github.com/dtolnay/thiserror) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/thiserror-2.0.21.txt) |
| thiserror-impl 2.0.21 | [thiserror-impl](https://github.com/dtolnay/thiserror) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/thiserror-impl-2.0.21.txt) |
| unicode-ident 1.0.27 | [unicode-ident](https://github.com/dtolnay/unicode-ident) | (MIT OR Apache-2.0) AND Unicode-3.0；选择 MIT；[完整文本](../../tools/boot/licenses/unicode-ident-1.0.27.txt)，另保留 [Unicode 声明](../../tools/boot/licenses/unicode-ident-1.0.27-unicode.txt) |
| virtio-drivers 0.13.0 | [virtio-drivers](https://github.com/rcore-os/virtio-drivers) | MIT；选择 MIT；[完整文本](../../tools/boot/licenses/virtio-drivers-0.13.0.txt) |
| volatile 0.4.6 | [volatile](https://github.com/rust-osdev/volatile) | MIT OR Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/volatile-0.4.6.txt) |
| x86_64 0.15.5 | [x86_64](https://github.com/rust-osdev/x86_64) | MIT/Apache-2.0；选择 MIT；[完整文本](../../tools/boot/licenses/x86_64-0.15.5.txt) |
| zerocopy 0.8.63 | [zerocopy](https://github.com/google/zerocopy) | BSD-2-Clause OR Apache-2.0 OR MIT；选择 MIT；[完整文本](../../tools/boot/licenses/zerocopy-0.8.63.txt) |
| zerocopy-derive 0.8.63 | [zerocopy-derive](https://github.com/google/zerocopy) | BSD-2-Clause OR Apache-2.0 OR MIT；选择 MIT；[完整文本](../../tools/boot/licenses/zerocopy-derive-0.8.63.txt) |

包体来自 crates.io 官方 registry，checksum 由 Cargo.lock 固定；精确包索引、原始 / 复制许可摘要保存在 evidence/t03-preflight-20261025-63e2193/。保留全部 22 包的 MIT 文本及 unicode-ident 的 Unicode 声明，共 23 份；为保守保留归属，构建阶段宏包的许可也随本地 ESP 的 /licenses/ 保存，不声称这些包全部链接进 guest。virtio-drivers 上游 LICENSE 缺末尾换行，本地仅补 LF；limine 沿用 T02 的相同处理，其余复制字节不变。完整 8 盘复核确认这些文件与正式源逐字相同。

本次未分发第三方源码、工具或镜像。原始许可文件不由摘要代替，未来发行仍需按最终链接 / 打包组成复核并确定原创许可。

T03 提交复核时，`git diff --cached --check` 对 zerocopy 与 zerocopy-derive 的原始 MIT 文本末尾空行给出提示；两份与 registry 包原文逐字相同，保留上游格式，不修改许可材料以消除该提示。此前工作树空白检查不包含尚未跟踪的新文件；代码与其余文本没有空白错误。

## T04 已引入的 GPT 组件

2026-10-25 按授权新增下列 8 个精确 registry 包，保留既有 22 个包版本与摘要。gpt_disk_io 关闭默认 feature，不启用 std / alloc；纯存储契约可在宿主测试，硬件依赖仍仅用于裸机。Cargo.lock 固定包体 checksum，xtask 仍仅使用 std。

| 包 / 精确版本 | 官方来源 | 上游许可与本次保留 |
| --- | --- | --- |
| gpt_disk_io 0.17.0 | [gpt_disk_io](https://github.com/google/gpt-disk-rs)；[crate](https://crates.io/crates/gpt_disk_io/0.17.0) | MIT OR Apache-2.0；保留 MIT；[完整文本](../../tools/boot/licenses/gpt_disk_io-0.17.0.txt) |
| gpt_disk_types 0.16.1 | [gpt_disk_types](https://github.com/google/gpt-disk-rs)；[crate](https://crates.io/crates/gpt_disk_types/0.16.1) | MIT OR Apache-2.0；保留 MIT；[完整文本](../../tools/boot/licenses/gpt_disk_types-0.16.1.txt) |
| bytemuck 1.25.2 | [bytemuck](https://github.com/Lokathor/bytemuck)；[crate](https://crates.io/crates/bytemuck/1.25.2) | Zlib OR Apache-2.0 OR MIT；保留 MIT；[完整文本](../../tools/boot/licenses/bytemuck-1.25.2.txt) |
| bytemuck_derive 1.12.1 | [bytemuck_derive](https://github.com/Lokathor/bytemuck)；[crate](https://crates.io/crates/bytemuck_derive/1.12.1) | Zlib OR Apache-2.0 OR MIT；保留 MIT；[完整文本](../../tools/boot/licenses/bytemuck_derive-1.12.1.txt) |
| crc 3.4.0 | [crc](https://github.com/mrhooray/crc-rs.git)；[crate](https://crates.io/crates/crc/3.4.0) | MIT OR Apache-2.0；保留 MIT；[完整文本](../../tools/boot/licenses/crc-3.4.0.txt) |
| crc-catalog 2.5.0 | [crc-catalog](https://github.com/akhilles/crc-catalog.git)；[crate](https://crates.io/crates/crc-catalog/2.5.0) | MIT OR Apache-2.0；保留 MIT；[完整文本](../../tools/boot/licenses/crc-catalog-2.5.0.txt) |
| ucs2 0.3.3 | [ucs2](https://github.com/rust-osdev/ucs2-rs)；[crate](https://crates.io/crates/ucs2/0.3.3) | MPL-2.0；保留 MPL-2.0；[完整文本](../../tools/boot/licenses/ucs2-0.3.3.txt) |
| uguid 2.2.1 | [uguid](https://github.com/google/gpt-disk-rs)；[crate](https://crates.io/crates/uguid/2.2.1) | MIT OR Apache-2.0；保留 MIT；[完整文本](../../tools/boot/licenses/uguid-2.2.1.txt) |

上游源文件未修改、未整包复制入仓库；许可从经 Cargo 校验的原包复制，仅对缺末尾换行的文本补 LF，原始与复制摘要见 evidence/t04-preflight-20261025-a37b3ef/license-copies.json。全部 30 个外部包对应 31 份许可文本（含 Unicode 声明），随本地 ESP 保存。ucs2 是 MPL-2.0，不将其归为 MIT；未来对外分发需按实际制品履行其源码提供和告知义务。当前仍无对外分发，原创许可未定。

## T05 FAT 候选与来源

2026-10-25 用户确认后，仅新增 rust-fatfs 的 fatfs 0.4.0 Git 包，固定 commit `38b3a7950bf0e765e34286d1dfe76c519ee347e2`，并更新 Cargo.lock。来源为[上游固定提交](https://github.com/rafalh/rust-fatfs/tree/38b3a7950bf0e765e34286d1dfe76c519ee347e2)，包内声明 MIT；[完整许可文本](../../tools/boot/licenses/fatfs-0.4.0.txt)逐字复制，随本地 ESP 保存。无上游源码修改，无整包源码迁入正式源，无对外分发。

关闭默认 feature，仅启用 alloc / lfn / unicode；不启用 std、chrono 或默认日志等级。复用既有 bitflags 2.13.2 与 log 0.4.34，保留全部 30 个 registry 包版本和 checksum；合计 31 个外部包、32 份许可文本。no_std 使用库内 IO traits，无新增 core_io / byteorder。v0.3.6 的 no_std 路线涉及不同 IO 依赖，因此本批选用精确 commit，不跟随 master，也未宣称已验证 v0.3.6 的工具链兼容性。

来源、commit、feature 及原始 / 复制许可摘要保存于 `evidence/t05-preflight-20261025-4bacb57/fatfs-identity.json`；Cargo.lock 固定 Git 来源，真实编译 / 运行结果与候选冻结状态以 current 指向的 T05 证据为准。Git commit 固定不等于对所有 FAT 用法的安全或兼容承诺；当前仅验证项目 DATA 接口及合成材料。
