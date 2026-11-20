<p align="center">
  <img src="images/EH3D_PHOENIX_3DP_BANNER.svg" alt="EH3D-PHOENIX-3DP banner" width="100%">
</p>

# EH3D-PHOENIX-3DP
<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | <a href="README_ita.md">🇮🇹 Italiano</a> | <a href="README_deu.md">🇩🇪 Deutsch</a> | 🇨🇳 <b>中文</b> | <a href="README_jpn.md">🇯🇵 日本語</a> | <a href="README_por.md">🇵🇹 Português</a></p>

### 准备并发送 3D 打印：模型、打印机配置、安全的 G 代码，以及与你的打印机和 EH3D 服务器的连接

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.0.2-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-scaffolding-FFB020.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-388-4CAF50.svg" alt="Tests">
</p>

---

**诚实检查 - 今天真正能运行的内容：** 一个由八个库和一个命令行程序（`phoenix-3dp`）组成的 Rust 工作区，带有 388 个可在电脑上运行的测试。它读取并检查 STL、OBJ 和 3MF 模型，包含一个由公开数据建立的 120 个打印机配置的目录（没有一个在机器上试过），为 Marlin、Klipper、RepRapFirmware 和 Bambu 方言写出 G 代码，温度上限会拒绝超限的值而不是截断它，并能操作 Moonraker 和 OctoPrint 打印机以及 EH3D 服务器，但仅限模拟器（内存中的，以及本机的套接字）。它能把封闭模型切成平整的层，生成外壁、直线填充和实心表层（一个小型切片器，有网格和螺旋体填充、桥接、简易支撑、接缝、可变线宽、移动时抬升，以及带加速度的时间估算；尚无多材料功能）；窗口尚未构建，也没有在屏幕上看过；没有任何内容在真实打印机上运行过。

---

> **安全状态：** 本程序不会单独控制机器，也不是安全系统。它的输出必须先由人检查，然后才能在打印机上运行；这里的任何内容都没有在真实硬件上运行过。

> **第三方名称：** 打印机厂商、型号和协议的名称（Bambu Lab、Prusa、Creality、Klipper、Moonraker、OctoPrint、Marlin 等）归其所有者所有，这里只用来说明指的是哪台机器或哪种协议；本程序不是由它们制作、认可或赞助的。它依据公开文档和事实编写，没有使用任何其他切片软件的代码、文字、图标或配置文件；名称、图形和文字均为自有。


## 📋 速览

| | |
|---|---|
| 版本 | `0.0.2` |
| 成熟度 | 脚手架 |
| 测试 | 388 |
| 技术栈 | Rust |
| 真实硬件 | 未试验：这里的任何内容都没有在打印机上运行过，包括 Bambu Lab A1 和 P1S。 |

**跳转到：** [概述](#-概述) · [架构](#-架构) · [快速开始](#-快速开始) · [测试](#-测试) · [结构](#-仓库结构) · [相关项目](#-相关项目)

## 🚦 项目状态

每一行是本仓库的一个部分，灯号表示它今天做到了哪一步。内容依据仓库能够展示的证据（测试、构建、运行），而不是计划。

🟢 已完成并验证 · 🟡 可用，有已知限制 · 🟠 部分完成或尚未验证 · 🔴 未完成或不可用

| 领域 | 状态 | 在这里的含义 |
|---|---|---|
| 核心 | 🟡 | 以纳米为单位的长度、网格及其检查、读取器的上限、一个小型 JSON 读取器和一个绝不会打印出来的机密值；已在电脑上用正常和损坏的输入测试。 |
| 模型和文件 | 🟡 | 读取 STL（文本和二进制）、OBJ 和基础 3MF，带有大小限制，遇到不支持的内容会指明并拒绝；写出自有的 3MF 包、二进制 STL、OBJ 和 G 代码文件；其他程序的 3MF 项目会读取其中的对象、构建板和部分设置（只读）；已用模糊测试检查，没有用其他程序的真实文件集合检查。 |
| 打印机配置 | 🟠 | 由公开数据建立的 120 个型号（打印空间、喷嘴、方言、连接方式、前 46 个型号为厂商标称的上限（包括 Bambu Lab A1 和 P1S），其余为程序的保守上限，各配置会注明是哪一种；六种通用耗材会对照打印机和喷嘴检查）；没有一个在机器上试过，所以全部标为 `verified = false`。 |
| G 代码 | 🟡 | 四种方言的写出器，带有安全的开头和结尾；超过上限的温度会被拒绝且不写入任何内容；检查器能找出程序中超限的行；从未在打印机上运行过。 |
| 打印机驱动 | 🟠 | Moonraker 和 OctoPrint 通过传输层工作，只用它们协议的模拟器试过；Bambu LAN 是标为实验性的设计，不会连接。 |
| 服务器连接 | 🟠 | 连接的设置，以及建立在套件通用客户端（登录、续期、重试、八种语言的错误信息）之上的服务器打印机插件客户端，已针对内存中的服务器试过；从未对真实服务器试过。 |
| 切片器 | 🟡 | 封闭网格的平整层、外壁、直线、网格或螺旋体填充、实心表层、桥接、简易支撑、接缝、可变线宽、按方言生成的 G 代码、以数据形式提供的逐层预览，以及带加速度的时间估算；已用固定种子形状上的性质测试检查（挤出的塑料量等于零件体积，没有任何内容超出热床，没有温度超过上限）；还没有用它在打印机上打印过。 |
| 命令行 | 🟡 | `phoenix-3dp`（`printers`、`info`、`check-profile`、`convert`、`filaments`、`slice`、`project`、`version`）运行库中的函数并打印结果；每个错误都以消息和非零退出码返回。 |
| 测试 | 🟡 | 388 个测试通过（`cargo test --workspace`），包含正常情况、边界情况、损坏的输入，涉及随机的地方使用固定种子。 |
| 安全 | 🟡 | 纯安全 Rust（`forbid(unsafe_code)`）：上限是拒绝而不是截断；令牌绝不打印；收到的文件不会引发 panic；没有独立审查。 |
| 窗口 | 🔴 | 四个标签页（准备、预览、设备、项目）的第一版已用 `--features gui` 写好；其逻辑已测试，但尚未构建，也没有在屏幕上看过。 |
| 真实硬件 | 🔴 | 未试验：这里的任何内容都没有在打印机上运行过，包括 Bambu Lab A1 和 P1S。 |
| 文档 | 🟡 | 八种语言的 README、变更日志、`docs/` 文件夹（架构、路线图、尚未完成的内容、服务器）以及由 `docs/` 绘制的两张图。 |

## 🎯 概述

* **理念：** PHOENIX-3DP 是 PHOENIX 家族中的 3D 打印程序。EH3D 服务器及其客户端是整个套件的中心：本程序只是服务器的又一个客户端，直接与打印机通信的独立模式是默认关闭的选项。
* **模型**（`p3dp-formats`）：读取 STL（文本和二进制）、OBJ 和基础 3MF，对大小、三角形数量和 ZIP 包内容设有限制；读不了的内容会带着所发现内容的名称被拒绝。可写出二进制 STL、OBJ、自有的 3MF 包和 G 代码文件。其他程序保存为 3MF 的项目，会读取其中的对象、构建板和部分设置（只读，并设有防范恶意包的限制）。
* **配置**（`p3dp-profiles`）：一个包含 120 款打印机（FDM 和树脂）的目录，依据公开事实编写：打印空间、喷嘴、方言、连接方式、多材料系统，以及最高温度（前 46 个型号为厂商标称值，其余为程序的保守上限，每个配置都会注明是哪一种）。六种通用耗材（PLA、PETG、ABS、ASA、TPU 和 PA）会对照打印机和喷嘴检查：不合适的组合会连同原因一起被拒绝。每个配置都会说明是否在机器上试过；目前没有一个试过。
* **G 代码**（`p3dp-gcode`）：Marlin、Klipper、RepRapFirmware 和 Bambu 的写出器，各自带有安全的文件头、开头和结尾；不允许的温度或移动会被拒绝，绝不截断。检查器可读取任何程序，找出超过型号上限的行。
* **连接**（`p3dp-link`）：一个驱动 trait、一套规范化状态（十五种状态、温度、作业、警报）和一份命令列表；Moonraker 和 OctoPrint 基于传输层实现（仅用标准库编写的普通 HTTP，带时间和大小限制，已在本机套接字上试过），并针对各协议的内存模拟器测试；Bambu LAN 是其主题和消息的设计，标为实验性。
* **服务器**（`p3dp-server-bridge`）：设置项 `server_address`、`server_token`、`server_machine`、`server_timeout_s`、`server_retries` 和 `server_mdns`，以及建立在套件通用客户端（`eh3d-serverclient`）之上的服务器 `printer3d` 插件客户端：作业进入队列并等待有人启动，温度上限在发送任何内容之前检查，并提供停止；已用内存中的服务器测试。
* **切片器**（`p3dp-slicer`）：封闭网格的平整层、外壁、带实心表层的直线、网格或螺旋体填充、桥接、简易支撑、接缝、可变线宽、移动时抬升、通过程序的写出器生成各方言的 G 代码，以及以数据形式提供的逐层预览和使用加速度的时间估算；放不进热床、超过温度上限或不是封闭网格的内容会被拒绝，绝不截断。
* **查看器**（`p3dp-viewer`）：不含图形代码的 3D 查看器模型：透视的环绕相机、在热床上的点击选择和框选、层的范围，以及供显卡使用的计划线条。用 wgpu 绘制的部分在默认关闭的功能之后，从未构建过。
* **其他文件：** 二进制 G 代码（`bgcode`）和开放的树脂层包（`.sl1`）只读取文件头，并给出曝光时间估算；树脂打印机的封闭格式会连同原因一起被拒绝。
* **尚未完成：** 3D 查看器的绘制（其模型已有）、多材料切片器、Bambu LAN 连接以及其他传输层。路线图在 `docs/ROADMAP.md`，缺少的内容在 `docs/NOT_YET.md`。

## 🧩 在 EH3D 中的角色

EH3D-PHOENIX-3DP 是 EH3D Fabrication Suite 中 PHOENIX 家族的 3D 打印程序。目前它是一组库加一个命令行；窗口用 `--features gui` 构建。服务器 `eh3d-server` 及其客户端是套件的中心：打印机、队列和权限预计放在服务器里（由那里的 `printer3d` 插件负责），本程序通过套件通用的设置与它通信。参见 [docs/SERVER.md](docs/SERVER.md)。

## 🏛️ 架构

<p align="center">
  <img src="images/architecture.svg" alt="EH3D-PHOENIX-3DP 的架构：谁在使用、包含什么、依赖什么" width="100%">
</p>


本仓库各部分如何配合：谁使用它、它包含什么、它依赖什么。图由 `docs/architecture.json` 绘制并与代码核对。

## ✨ 功能

* **上限就是拒绝：** 不允许的温度或移动是一个指明数值和上限的错误，绝不会被截断到上限；写出器、驱动和检查器都遵守。
* **不会自行开始：** 发往服务器队列的请求带有 `auto_start: false` 并要求确认；原始 G 代码默认关闭，开启后也按同样的上限逐行处理。
* **所有协议一种状态：** 十五种规范化状态、温度、作业和警报，打印机的原始文本单独保存，使程序其余部分只说一种语言。
* **测试用模拟器：** 每个协议都有可按需制造故障（无连接、超时、503）的内存模拟器，因此重试、拒绝和关机都无需网络即可测试。
* **净室与自有图形：** 依据公开协议、手册和事实编写；不使用其他切片软件的代码、文字、图标或配置文件；在试用之前每个配置都标为 `verified = false`。
* **八种语言：** 程序文字和文档提供英语、西班牙语、德语、法语、意大利语、日语、中文和葡萄牙（葡萄牙）语版本。
* **没有 unsafe 代码：** 每个 crate 都有 `#![forbid(unsafe_code)]`。

## 🔒 安全与防护

| 威胁 | 防御 |
|---|---|
| 损坏、被截断或恶意的模型文件 | 分配任何内存之前先检查大小；三角形、顶点、范围、ZIP 包的条目数及其解压后的大小都有上限，压缩炸弹会被拒绝；模糊测试在每个位置截断有效文件并输入乱码，任何情况都不得 panic |
| 内部藏有意外内容的包（组件、其他文件中的对象、声明的实体） | 会指明并拒绝，绝不跳过；声明的 XML 实体是错误 |
| 超过型号允许的温度 | 连同数值和上限一起被拒绝；绝不截断到上限；写出器、驱动和检查器遵守同一规则 |
| 绕过上限的原始 G 代码行 | 默认关闭；逐行处理；与其他一切使用相同的上限检查 |
| 进入路径或命令的文件名或标识 | 不允许文件夹、`..`、控制字符，只接受已知扩展名，进入路径的内容做百分号编码 |
| 日志或屏幕上的服务器令牌或打印机密钥 | 保存在会打印 `***` 的类型中；显示设置时令牌被隐藏 |
| 自行启动作业的队列 | 请求带有 `auto_start: false` 并要求确认；服务器能否自行启动作业是服务器的策略 |
| 不稳定的网络 | 只在不会发生两次的地方重试：连接被拒绝时任何请求都会重试，超时仅对可以重发的请求重试，POST 绝不重试 |
| 不安全的内存使用 | 使用 `forbid(unsafe_code)` 的纯安全 Rust |

**不在防护范围内的内容：** 写得干净的程序不等于可以放心运行的程序。上限是厂商标称的值，没有在机器上核对过；驱动只接触过模拟器；没有经过独立审查。请私下报告漏洞（见 [SECURITY.md](SECURITY.md)）。

## 🔌 接口

库有 `p3dp_core`、`p3dp_formats`、`p3dp_profiles`、`p3dp_gcode`、`p3dp_link`、`p3dp_server_bridge`、`p3dp_slicer` 和 `p3dp_viewer`；程序是库 `eh3d_phoenix_3dp` 和命令行 `phoenix-3dp`。每个命令把结果打印到标准输出，并把错误连同非零退出码报告到标准错误（所请求的操作失败为 1，命令写错为 2）。唯一的网络访问是驱动和服务器连接的传输层，且只访问给定的地址。

```text
phoenix-3dp printers [--vendor NAME] [--tech fdm|msla] [--search TEXT]
phoenix-3dp info MODEL [--printer ID]
phoenix-3dp check-profile ID [--gcode FILE]
phoenix-3dp check-profile --file PROFILE [--gcode FILE]
phoenix-3dp convert IN OUT [--printer ID] [--force]
phoenix-3dp filaments [--printer ID] [--nozzle MM] [--hardened]
phoenix-3dp slice MODEL --printer ID [--settings FILE] [--filament ID] [--nozzle MM] [--hardened] [--out FILE.gcode] [--force]
phoenix-3dp project FILE.3mf|FILE.bgcode|FILE.sl1 [--bottom-layers N] [--lift MM]
phoenix-3dp version
phoenix-3dp gui [SETTINGS]                       the window (a build with --features gui)
```

## ⚙️ 配置

设置是一个由 `键 = 值` 行组成的文件（`--config 文件`）：读取器上限（`limit_*`）、与服务器的连接（`server_address`、`server_token`、`server_machine`、`server_timeout_s`、`server_retries`、`server_mdns`、`server_max_answer_mib`）、`direct_mode`（独立模式，默认关闭）、`lang` 和 `default_printer`。打印机配置是同类的另一个文件（`examples/my-printer.cfg`）。未知的键或超出范围的值都是会指明所在行的错误；所有默认值都偏保守。参见 `examples/phoenix-3dp.cfg`。

## 🚀 快速开始

需要稳定版 Rust 工具链（1.88 或更新）。请先在本仓库旁边克隆 EH3D-COMMON：服务器的通用客户端来自那里。

```bash
cargo build --release
cargo test --workspace
cargo run --release -- printers --search bambu
cargo run --release -- info examples/cube-20mm.stl --printer bambu-lab-a1
cargo run --release -- check-profile --file examples/my-printer.cfg --gcode examples/job-too-hot.gcode
cargo run --release -- convert examples/cube-20mm.stl cube.3mf --printer prusa-mk4s
cargo run --release --features gui -- gui   # the window
```

## 🧪 测试

测试套件有 388 个测试（`cargo test --workspace`，CI 中在 Windows 和 Linux 上运行），涉及随机的地方使用固定种子。它们检查正常情况、边界情况和损坏的输入：被截断的文件、乱码和恶意文件头绝不会 panic，目录中任何打印机的温度都不会超过其上限。CI 还会运行 `cargo fmt --all --check`、把警告当作错误的 `cargo clippy` 以及 `python -I tools/ci_validate.py`。

## 🏗️ 构建与发布

版本记录在 `Cargo.toml` 和 `eh3d.project.json` 中，每个版本在 [CHANGELOG.md](CHANGELOG.md) 中都有条目。`build.bat` / `build.sh` 构建程序，`test.bat` / `test.sh` 运行测试，`run.bat` / `run.sh` 带示例运行。发布版本会放在 GitHub 上。目前尚未发布任何版本。

## 📂 仓库结构

<p align="center">
  <img src="images/repository-structure.svg" alt="EH3D-PHOENIX-3DP 的仓库结构" width="100%">
</p>

<details>
<summary>目录树的文本版</summary>

```text
EH3D-PHOENIX-3DP/
├── crates/
│   ├── p3dp-core/                               单位、网格及检查、上限、JSON、设置
│   ├── p3dp-formats/                            STL、OBJ、3MF、项目、bgcode 和树脂的读取器；写出器、ZIP
│   ├── p3dp-profiles/                           依据公开事实的打印机、耗材、配置文件
│   ├── p3dp-gcode/                              四种方言的 G 代码写出器和检查器
│   ├── p3dp-link/                               驱动、规范化状态、TCP、模拟器、Bambu LAN
│   ├── p3dp-server-bridge/                      服务器连接设置和插件客户端
│   ├── p3dp-slicer/                             切片器：层、外壁、填充、桥接、支撑、G 代码
│   └── p3dp-viewer/                             3D 查看器的模型：相机、选择、层
├── src/
│   ├── lib.rs                                   程序的库：命令行和窗口使用的部分
│   ├── main.rs                                  命令行和窗口的启动
│   ├── cli.rs                                   命令：printers、info、check-profile、convert、filaments、slice、project、version
│   ├── app_state.rs                             无窗口的窗口状态：标签页、打印机、模型、报告、模拟设备
│   ├── config.rs                                设置文件：上限、服务器连接、语言和默认打印机
│   ├── i18n.rs                                  程序的八种语言文字
│   ├── project.rs                               项目：模型及其打印机，保存在自有的 3MF 包中
│   ├── view.rs                                  把网格变成预览线条的轨道相机
│   └── gui.rs                                   窗口（egui，gui 功能）：绘制状态并转达用户的操作
├── tests/
│   ├── end_to_end.rs                            从模型到程序再到模拟打印机的测试
│   ├── examples.rs                              测试 examples/ 中的文件能配合命令使用
│   ├── fuzz.rs                                  固定种子的模糊测试：截断、损坏和恶意文件
│   ├── readme.rs                                测试 README 所述属实：测试数量和命令
│   └── safety.rs                                测试目录中任何打印机的温度都不会超过其上限
├── examples/
│   ├── cube-20mm.stl                            20 毫米立方体（文本 STL）
│   ├── cube-20mm.obj                            同一个立方体（OBJ）
│   ├── phoenix-3dp.cfg                          含所有键及其默认值的设置文件
│   ├── my-printer.cfg                           手写的打印机配置（一台 Klipper 机器）
│   ├── job-safe.gcode                           遵守该配置上限的程序
│   └── job-too-hot.gcode                        有两行超过上限的程序
├── docs/
│   ├── ARCHITECTURE.md                          按 crate 说明程序
│   ├── ROADMAP.md                               从基础到完整程序的九个步骤
│   ├── NOT_YET.md                               尚未完成的内容及各部分的需求
│   ├── SERVER.md                                程序如何与 EH3D 服务器配合
│   ├── architecture.json                        架构图数据
│   └── structure.json                           结构图数据
├── tools/                                       CI 检查、README 检查和图片工具
├── images/                                      横幅图和 README 的两张图
├── .github/                                     CI 与发布工作流、议题和拉取请求模板
├── eh3d.project.json                            清单：版本、角色、成熟度和构建命令
├── Cargo.toml                                   工作区及其 crate
├── rust-toolchain.toml                          固定的 Rust 工具链
├── CHANGELOG.md                                 每个版本的变更
├── SECURITY.md SUPPORT.md                       如何报告漏洞以及在哪里提问
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   如何贡献、行为准则和许可证
```

</details>

## 🗺️ 路线图与已知限制

**目前已知的限制**

* 切片器很小：支撑是简易的（没有树状支撑和界面层），桥接没有调校，没有多材料功能，也没有用它在打印机上打印过。
* 还没有连接过任何打印机，包括 Bambu Lab A1 和 P1S；每个配置都标为 `verified = false`。
* 3MF 读取器只取网格（组件和生产扩展会被拒绝）；其他程序的项目只读取其对象、构建板和设置，绝不修改。
* 驱动有一个普通 HTTP 传输层，只在本机套接字和模拟器上试过；没有 TLS，也没有 WebSocket 推送；Bambu LAN 是不会连接的设计，Bambu 云不在其范围内。
* 窗口尚未构建，也没有在屏幕上看过。

**下一步**

* H1：网格几何和自有的 3D 查看器。
* H2：在真机上打印的校准立方体（网格和螺旋体填充、接缝、可变线宽、桥接和简易支撑已编写，并用固定种子的形状测试过）。
* H3：配置编辑器和基础窗口。
* H7：其他传输层（WebSocket 推送、PrusaLink、Duet）和 Bambu LAN 连接。
* H8：在 Bambu Lab A1 和 P1S 上试用，并把可用的配置标为已验证。完整计划见 `docs/ROADMAP.md`。

## 🌍 语言

README 和文本提供英语、西班牙语、法语、意大利语、德语、汉语、日语和欧洲葡萄牙语版本。

## 🔗 相关项目

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON): 共享的 crate，其中包括服务器的通用客户端
* [EH3D-SERVER](https://github.com/JuanenRac/EH3D-SERVER): 套件的服务器，打印机、队列和权限预计放在这里
* [EH3D-STUDIO](https://github.com/JuanenRac/EH3D-STUDIO): 套件的配置面板
* [EH3D-PHOENIX-LASER](https://github.com/JuanenRac/EH3D-PHOENIX-LASER): 激光雕刻和切割，同一家族的姊妹程序
* [EH3D-PHOENIX-PNP](https://github.com/JuanenRac/EH3D-PHOENIX-PNP): 贴片机（拾取与放置），同一家族的姊妹程序
* [EH3D-SUITE-LAUNCHER](https://github.com/JuanenRac/EH3D-SUITE-LAUNCHER): 查找并启动套件中各程序的中心

## 📚 文档

* [变更日志](CHANGELOG.md)
* [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): 程序架构，按 crate 说明
* [docs/ROADMAP.md](docs/ROADMAP.md): 路线图：从基础到完整程序的九个步骤
* [docs/NOT_YET.md](docs/NOT_YET.md): 尚未完成的内容，以及每个缺失部分的需求
* [docs/SERVER.md](docs/SERVER.md): 程序如何与 EH3D 服务器配合
* [docs/architecture.json](docs/architecture.json): 两张图的数据

## 🤝 参与贡献与社区

* 提交拉取请求前请运行 `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 和 `python -I tools/ci_validate.py`：必须全部通过。见 [CONTRIBUTING.md](CONTRIBUTING.md) 和[行为准则](CODE_OF_CONDUCT.md)。
* README 有八种语言：修改文字要在八个文件中同时进行，两张图由 `tools/make_architecture.py` 和 `tools/make_structure.py` 根据 `docs/architecture.json` 和 `docs/structure.json` 绘制。
* 请私下报告漏洞：[SECURITY.md](SECURITY.md)。问题、想法和报告：electrohobby3d@gmail.com（[SUPPORT.md](SUPPORT.md)）。

## 👤 作者

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 许可证

软件：GPL-3.0-or-later。见 [LICENSE](LICENSE)。套件的硬件设计使用 CERN OHL-S。
