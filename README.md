# driver-razer

WaiShe 的 Razer 设备驱动：用户态的协议适配器，由 WaiShe Bridge 加载。不是操作系统驱动。

**当前阶段：0.1.0，DeathAdder V3 Pro 的 2.4G 接收器模式已在 WaiShe Bridge 中实机验收（M1，2026-11-14）。有线模式尚未在本项目验证。** 构建：`cargo build --release --target wasm32-unknown-unknown`；测试：先构建再 `cargo test`；打包：用 `waishe-driver-sdk` 的 `waishe-driver-dev pack`。

## 设备支持

下表是协议层面的实机验证结果（2026-11-14，固件 v2.2），不代表已有可用驱动。

| 设备 | 连接方式 | 电量 | 充电状态 | 连接状态 | DPI | 回报率 |
| --- | --- | --- | --- | --- | --- | --- |
| DeathAdder V3 Pro | 有线 | 未测 | 未测 | 未测 | 未测 | 未测 |
| DeathAdder V3 Pro | 2.4G 接收器 | verified | verified（仅“未充电”） | verified | verified | verified（1000、500 Hz） |

验证等级：`verified` 实机验证 / `simulated` 回放测试 / `inferred` 同协议族推断 / `broken` 已知不可用。

每款设备有一份验证计划与记录：

- [DeathAdder V3 Pro](docs/deathadder-v3-pro.md)

## 范围

首期只做 DeathAdder V3 Pro 的电量、连接状态、DPI、回报率，以及协议支持时的充电状态。

不做：固件升级、按键映射、灯光、宏。固件类命令永远不在范围内。

驱动按协议族组织。Razer 鼠标之间共享大部分协议，后续型号以设备表条目的形式加入，必要时才增加代码分支。

## 使用注意

Razer Synapse 可能与本驱动同时向设备发送命令。M0 实测中 Synapse 运行时读写均正常，10 秒内未被覆盖；更长时间的共存尚未测试，见设备文档。

## 协议来源

见 [PROVENANCE.md](PROVENANCE.md)。任何协议代码合并前必须先在其中登记来源。

## 许可证

[AGPL-3.0-or-later](LICENSE)。

选择该许可证是因为本驱动预期参照 OpenMouse（AGPL-3.0）与 openrazer（GPL-2.0-or-later）的实现。WaiShe 的其他核心仓库采用宽松许可证，本仓库的代码不得复制到那些仓库中。详见 `waishe` 仓库决策记录 0002。

## 相关

- 驱动接口与能力模型：`waishe-driver-sdk` 仓库。
- 项目总览：`waishe` 仓库。
- 贡献流程：`.github` 仓库的 `CONTRIBUTING.md`。

## 声明

本项目与 Razer Inc. 没有关联，也未获其认可或赞助。Razer 及相关产品名称是其所有者的商标，仅用于指明兼容对象。
