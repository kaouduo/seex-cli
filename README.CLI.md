# SeEx CLI 本地改版

SeEx by Ref42：https://github.com/ref42/seex 。保留原 LICENSE；本地改动见 UPSTREAM.md。

本工具复用 seex 的网络接口、符号/封装转换和模型下载代码。原 GUI 仍为
`seex.exe`，新增控制台程序为 `seex-cli.exe`，没有安装 MCP 服务。

## Windows 编译

需要 Rust stable `x86_64-pc-windows-msvc`、Visual Studio C++ Build Tools 与 Windows SDK。
仅运行已编译 exe 无需安装 Rust 或 C++ 编译器。首次构建需联网获取 Cargo 依赖。

```powershell
cd path\to\seex-cli
.\build-cli.ps1
.\dist\seex-cli.exe --help
```

GUI 可另外执行 `cargo build --release --features gui --bin seex`。改版不修改 GUI 的原有交互。
CLI 默认不编译图形界面依赖。

## 命令

```powershell
# 读取立创专业版 ZIP 工程或 epru，不修改输入文件
.\dist\seex-cli.exe scan-project 'D:\project\board.epro2' -o inventory.json --ids-output ids.txt

# 查询型号、商城编号和线上资源 UUID
.\dist\seex-cli.exe search C2040

# 按商城编号下载模型
.\dist\seex-cli.exe download-step C2040 --output staging

# 原工程保存的 3D Model 属性为 32位UUID|所有者 时，可直接传入整段
.\dist\seex-cli.exe model-uuid '<32位线上模型UUID>' -o staging\part.step
# 若手中已是 STEP 资源 UUID，使用 --step-asset 跳过模型元数据解析

# 保存上游符号/封装 JSON；bundle 额外获取模型和来源清单
.\dist\seex-cli.exe source C2040 --output cache
.\dist\seex-cli.exe bundle C2040 --output cache

# 在独立暂存目录导出，不能直接导出到 KiCad 工程内
.\dist\seex-cli.exe kicad export C2040 --full -o staging --library-name ASP200
.\dist\seex-cli.exe kicad batch -i ids.txt --full -o staging --library-name ASP200 --continue-on-error
```

输出库存清单包含：页本地 ID、位号、实例/器件/符号/封装本地 ID、器件来源、
商城编号、3D Model 原始引用、模型变换与器件属性。C 编号列表按属性提取并去重；
**批量导出应输入 ids.txt，不要把 inventory.json 当作编号文本输入**。

工程实例 ID 和 16 位资源 ID 不能直接当作线上 32 位 UUID。
扫描器不会把位号或型号猜成商城编号，也不会自动选中搜索结果来替换工程器件。
没有商城编号的条目标为 needs_resolution，保留原始信息供后续处理。

执行结果 JSON 在 stdout，执行错误 JSON 在 stderr；帮助和参数解析错误为普通文本。
退出码：0 成功、1 执行失败、2 参数错误或部分结果（包括 KiCad 导出缺失模型）。
上游导出遇到错误可能留下部分暂存文件，失败不代表目录回滚。
扫描输出默认不覆盖已有文件；上游下载/转换仅在显式 --force 时覆盖对应输出。

模型下载不等于机械验收。符号编号、焊盘/槽孔、STEP 方向和偏移需要核查；
原工程的 transform 仅原样保留，没有自动套用到 KiCad。
本工具不移动 PCB 器件，不注册工程库，不关联板上模型，不写原理图/PCB。
暂存结果需核查后通过 KiCad/Konnect 接入工程。

当前扫描支持本项目所用的 EasyEDA Pro `.epro2`/`.epru` 格式；
不声称支持标准版 JSON、所有历史工程版本或 KiCad 实例 UUID 反查。

本次实际验证结果见 TEST_REPORT.md；单元测试可运行 `./test-cli.ps1`。
