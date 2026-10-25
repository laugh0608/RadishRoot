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
