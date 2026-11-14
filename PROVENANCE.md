# 协议来源记录

本文件登记本仓库中协议知识与代码的来源。规则：

1. 协议代码合并前，先在这里登记来源。没有登记的不合并。
2. 移植或改写了别人的代码，要写明来源文件和提交哈希，并保留原版权声明。
3. 只参考了文档或行为、没有照着源码写，也要登记，注明“仅参考”。
4. 来自自己的抓包，要写明设备、固件版本、抓包环境，并把去除了可识别信息的记录放进 `captures/`。

## 来源

| 来源 | 许可证 | 状态 | 使用方式 | 涉及文件 | 来源版本 |
| --- | --- | --- | --- | --- | --- |
| [OpenMouse mouse-protocol](https://github.com/OpenMouse-Project/mouse-protocol) `src/razer/codec.ts` | AGPL-3.0-or-later | **已移植** | 报文布局、校验、应答解码、各命令定义与解码函数（固件、序列号、电量、充电、DPI 读写、旧式回报率读写） | `src/codec.rs` | 提交 `5a4f6433da3d70865d008874c7055016b336c197` |
| [OpenMouse mouse-protocol](https://github.com/OpenMouse-Project/mouse-protocol) `src/razer/devices.ts` | AGPL-3.0-or-later | **已移植** | DeathAdder V3 Pro 的 `0x00B6`、`0x00B7`、`0x00C2` 条目：事务 ID、DPI 上限、回报率表、`0x00B7` 使用旧式回报率命令的结论；DPI 最小值与步进来自同仓库 `src/drivers/razer/hid.ts` | `src/devices.rs` | 同上 |
| [OpenMouse mouse-protocol](https://github.com/OpenMouse-Project/mouse-protocol) `src/drivers/razer/hid.ts` | AGPL-3.0-or-later | **已移植** | `exchange`、`awaitReply` 的重试规则（忙则续读、损坏则重发、只重发读命令）、应答等待 100 ms × 6 次、改回报率后等待 150 ms、`setPollingRate` 的回读确认 | `src/lib.rs` | 同上 |
| [OpenMouse mouse-protocol](https://github.com/OpenMouse-Project/mouse-protocol) `captures/razer-deathadder-v3-pro/` | AGPL-3.0-or-later | 仅参考 | 有线模式的验证依据，见 `devices.toml` 中 `inferred` 条目的说明 | `devices.toml` | 同上 |
| [openrazer](https://github.com/openrazer/openrazer) | GPL-2.0-or-later | 协议事实已用于实机验证，尚未引入代码 | 仅参考：`driver/razermouse_driver.h` 的产品 ID；`driver/razercommon.h` 的报文结构与应答状态；`driver/razercommon.c` 的校验算法；`driver/razerchromacommon.c` 的命令构造（固件、序列号、电量、充电、DPI 读写、回报率读写）；`driver/razermouse_driver.c` 中本设备使用的事务 ID `0x1F` 与命令选择 | `docs/deathadder-v3-pro.md` | 主分支，2026-11-14 读取 |
| 本项目抓包 | — | 已记录 | DeathAdder V3 Pro 固件 v2.2、接收器 `1532:00B7`，Windows 11 | `captures/deathadder-v3-pro-receiver-2026-11-14.md` | 2026-11-14 |

## 许可证兼容性

本仓库为 AGPL-3.0-or-later。

- AGPL-3.0 的代码可以直接引入。
- GPL-2.0-or-later 的代码可按 GPL-3.0 使用，再与 AGPL-3.0 代码组合。
- 仅 GPL-2.0（不含“或更高版本”）的代码不能引入。
- MIT、Apache-2.0 等宽松许可证的代码可以引入，保留其声明。
