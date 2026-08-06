<p align="center">
  <img src="assets/icons/seex.png" alt="SeEx 图标" width="160" />
</p>

<h1 align="center">SeEx</h1>

<p align="center">
  <strong>批量导出立创商城元件为Altium Designer（AD）、KiCad可用的格式。</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-66BB6A" alt="平台" />
</p>

<p align="center"><a href="README.en.md">English</a></p>

---

## 写在开头
> [!NOTE]
> 项目源码虽然已经完全可见，但请务必遵循[LICENSE](LICENSE)进行使用。


整个项目的开发历程（按照时间线）：
- 项目伊始，开发了`nlbn（Nice Library Bring now，谐音“拿来吧你”）`，期间曾开源该工具，也获得了不少的star，但还是被直接抄袭，甚至上传到`Gitee`（垃圾平台，请不要上传我的代码到此处，我认为这个平台污染了我的代码，即使此项目也借助了Agent之力。），遂停止了`nlbn`的维护。也移除了相关的`crate`。我也意识到其实大家根本不看`README`，也不会看`License`文件，特别是当初仅提供了英文版本的`README`，遂不再维护`nlbn`。
- 开发`nlbn`的同时，我也在尝试逆向`Altium Designer`的文件格式，逐渐开发出了`npnp（Normalize Pin Net Pad，谐音“牛批牛批”）`，也尝试了开源，但还是被抄袭。此时开发了相应的`GUI`版本，使用的框架是`tauri2`，虽然制作粗糙，但还是帮不少人解决了问题，我也很开心。
- 对应开发的`GUI`工具，也就是现在的`seex（seex & Export）`，到今天之前都还是直接调用`npnp`的`cli`进行导出，所以我不得不同时维护两个仓库，时间久了，也觉得累。所以今天进行了重构，`seex`不再依赖`npnp`，而是直接继承了`npnp`的功能，现在用的`GUI`框架是`egui/eframe`，也就是说，以后不再发布`npnp`的二进制文件了，从今开始仅维护`seex`。
- 如果你有想要实现的功能，可以跟我保持联系。我的邮箱是：`lampmud@163.com`/`ybinlew@gmail.com`。你也可以在`issue`里提出。
- 请不要再用`python`来重写我的工具了，`python`不是我的`taste`，感谢理解。

## SeEx现在能做啥？？？
### KiCad部分
- [x] 可以导出KiCad可用的格式，包括`Symbol`, `Footprint`, `3D`模型。
- [x] 可以批量导出以上格式，也可以单独导出。甚至你可以单独导出`3D`模型，仅`step`格式。
### Altium Designer（AD）部分
- [x] 可以单独导出器件的`schlib`, `pcblib`（`pcblib`已经内嵌了`step`模型）。
- [x] 也可以批量导出。
- [x] 支持导出多个器件到同一个`schlib`, `pcblib`，即合并导出。
### 共同部分
- [x] 均支持选择中英文元数据导出。
- [x] 均支持多线程下载。
- [x] 均支持向已现存的经由`seex`导出的库里添加新的器件/元件。

## SeEx未来想做啥？？？
- [ ] 支持更多的`EDA`工具，类似`allegro`, `pads`等。
- [ ] 添加`MCP`支持，未来肯定可以实现一句话让`Agent`自行搭建工程，人类做好`review`的工作即可，至少我是这样希望的。
- [ ] 继续完善现在的工作流，修复更多隐藏的`bug`，许多器件的封装还是需要在导出后进行微调，目前并没有更好的解决办法。
- [ ] 开发`Altium Designer(AD)`可用的插件，类似

## SeEx不会支持啥？？？
> [!NOTE]
> 1. 不会增加预览`3D`模型的功能。可能部分二开的工具确实增加了这个功能，但在我看来就是画蛇添足，直接在xx商城预览就行，根本没必要搬到`seex`里，一是占据了界面空间，二是占用内存。
> 2. 如果没有更好的`GUI`框架出现的话，可能不会在短时间内更换`GUI`框架。

### 导出示例

KiCad 导出会生成符号、封装和 3D 模型：

<table>
  <tr>
    <td align="center"><img src="imgs/kicad_symbol.png" alt="KiCad 符号" width="280" /><br /><sub>符号库</sub></td>
    <td align="center"><img src="imgs/kicad_footprint.png" alt="KiCad 封装" width="280" /><br /><sub>封装库</sub></td>
    <td align="center"><img src="imgs/kicad_3dmodel.png" alt="KiCad 3D 模型" width="280" /><br /><sub>3D 模型</sub></td>
  </tr>
</table>

<details>
  <summary>查看 Altium PCB 库示例</summary>

  <table>
    <tr>
      <td><img src="imgs/pcb_01.png" alt="Altium PCB 示例 1" width="420" /></td>
      <td><img src="imgs/pcb_02.png" alt="Altium PCB 示例 2" width="420" /></td>
      <td><img src="imgs/pcb_03.png" alt="Altium PCB 示例 3" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/pcb_04.png" alt="Altium PCB 示例 4" width="420" /></td>
      <td><img src="imgs/pcb_05.png" alt="Altium PCB 示例 5" width="420" /></td>
      <td><img src="imgs/pcb_06.png" alt="Altium PCB 示例 6" width="420" /></td>
    </tr>
  </table>
</details>

<details>
  <summary>查看 Altium SchLib 示例</summary>

  <table>
    <tr>
      <td><img src="imgs/sch_01.png" alt="Altium SchLib 示例 1" width="420" /></td>
      <td><img src="imgs/sch_02.png" alt="Altium SchLib 示例 2" width="420" /></td>
      <td><img src="imgs/sch_03.png" alt="Altium SchLib 示例 3" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/sch_04.png" alt="Altium SchLib 示例 4" width="420" /></td>
      <td><img src="imgs/sch_05.png" alt="Altium SchLib 示例 5" width="420" /></td>
      <td><img src="imgs/sch_06.png" alt="Altium SchLib 示例 6" width="420" /></td>
    </tr>
  </table>
</details>

### 4. 合并库文件

Altium 导出时打开合并选项，可以将多个元件写入同一个 SchLib/PcbLib 库。输入库名称后即可创建或追加到对应库文件。

KiCad 导出默认使用合并库模式，并在输出目录中创建符号库、封装库和 3D 模型目录。

### 5. 调整导出速度

使用并行数量控制同时处理的元件数量。数量越大通常速度越快，但也会增加网络请求和系统资源使用。建议先从 `4` 或 `8` 开始，再根据网络和电脑性能调整。

已存在的文件默认会被跳过。需要重新生成时打开强制覆盖选项。

## 发布版本

Windows 用户可以使用 x64 `setup.exe` 安装，也可以直接运行便携版 `.exe`（仅测试了`Windows`版本，因为日常主要使用`AD`。）。Windows ARM、Linux x64/ARM64 和 macOS Intel/Apple Silicon 版本可在 GitHub Releases 中下载对应压缩包。这些平台我都没有，只是白嫖了`Github Action`进行了多平台分发，具体平台的安装方法，请参考相应解决方案。

## 语言和设置

点击顶部语言按钮切换中文和 English。窗口置顶按钮可以让 SeEx 保持在其他应用上方。

## 参考和致谢

- 感谢[`easyeda2kicad`](https://github.com/uPesy/easyeda2kicad.py)@[uPesy](https://github.com/uPesy)给了我项目最开始的灵感，这是一个很优秀的工具，可以说`KiCad`相关的`easyeda`导入插件，绝大多数都是在这个工具的基础上进行开发的。
- 感谢群友菊猫赠送的一部分`TOKEN`，虽然也自费购买了一部分，但是最后算下来也属于半买半送了，那个时候猛蹬`opus 4.6`，实现了很多功能，打下了好的基础。
