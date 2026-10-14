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

CI 声明使用 GitHub 官方 actions/checkout v4.2.2，固定 commit `11bd71901bbe5b1630ceea73d27597364c9af683`，上游为 MIT：[release](https://github.com/actions/checkout/releases/tag/v4.2.2)、[LICENSE](https://github.com/actions/checkout/blob/v4.2.2/LICENSE)。本地未下载或执行该 Action；后续运行由 GitHub runner 获取，仅用于 CI，不进入系统镜像。宿主已有 Python 标准库用于治理检查，无新增 pip 依赖；它不是 guest 运行依赖。任何后续打包分发须按实际清单重新核对。
