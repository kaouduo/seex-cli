<p align="center">
  <img src="assets/icons/seex.png" alt="SeEx 图标" width="160" />
</p>

<h1 align="center">SeEx</h1>

<p align="center">
  <strong>批量导出立创商城元件为Altium Designer（AD）、KiCad可用的格式。</strong>
</p>

<p align="center"><a href="README.en.md">English</a> | <a href="README.md">简体中文</a></p>


---

## 写在开头
> [!NOTE]
> 项目源码虽已完全公开，但请务必遵循[LICENSE](LICENSE)进行使用。

整个项目的开发历程（按照时间线）：
- 项目伊始，开发了`nlbn（Nice Library Bring now，谐音“拿来吧你”）`，期间曾开源该工具，也获得了不少的star，但却被上传到`Gitee`（垃圾平台，请不要上传我的代码到此处，我认为这个平台污染了我的代码。），遂停止了`nlbn`的维护。我也意识到其实大家根本不看`README`，也不会看`License`文件，对于版权和别人的劳动更是零尊重，特别是当初仅提供了英文版本的`README`，很多人甚至是看不懂的，遂不再维护`nlbn`。
- 开发`nlbn`的同时，我也在尝试逆向`Altium Designer`的文件格式，逐渐开发出了`npnp（Normalize Pin Net Pad，谐音“牛批牛批”）`，也尝试了开源，但还是被抄袭。此时开发了相应的`GUI`版本，使用的框架是`tauri2`，虽然制作粗糙，但还是帮不少人解决了问题，我也很开心。
- 对应开发的`GUI`工具，也就是现在的`seex（seex & Export）`，一开始是直接调用`npnp`的`cli`进行导出，所以我不得不同时维护两个仓库，时间久了，也觉得累。所以今天进行了重构，`seex`不再依赖`npnp`，而是直接继承了`npnp`的功能，现在用的`GUI`框架是`egui/eframe`，也就是说，以后不再发布`npnp`的二进制文件了，从今开始仅维护`seex`。
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
- [ ] 添加`MCP`支持，更好玩，更省力。
- [ ] 继续完善现在的工作流，修复更多隐藏的`bug`。
- [ ] 持续优化交互功能，提升用户使用体验，优化图标，尝试新的设计语言。
- [ ] 探索一种普遍适用的方案，用于除去/隐去`3D`模型的一部分组件，类似特定标识符等等。

## SeEx期待的贡献者？？？
- [x] 具有软件设计经验，能持续维护软件。
- [x] 审美在线，非常期待得到您的宝贵建议。
- [x] “设计师”？？？可以帮助我们重新设计软件的图标。
- [x] 或许可以考虑重新给软件取名？？？

## SeEx不会支持啥？？？
> [!NOTE]
> 1. 不会增加预览`3D`模型的功能。可能部分二开的工具确实增加了这个功能，但在我看来就是画蛇添足，直接在xx商城预览就行，根本没必要搬到`seex`里，一是占据了界面空间，二是占用内存。
> 2. 如果没有更好的`GUI`框架出现的话，短时间内不会更换`GUI`框架。

### 导出示例

KiCad 导出的符号、封装和 3D 模型：

<table>
  <tr>
    <td align="center"><img src="imgs/kicad_symbol.png" alt="KiCad 符号" width="280" /><br /><sub>符号库</sub></td>
    <td align="center"><img src="imgs/kicad_footprint.png" alt="KiCad 封装" width="280" /><br /><sub>封装库</sub></td>
    <td align="center"><img src="imgs/kicad_3dmodel.png" alt="KiCad 3D 模型" width="280" /><br /><sub>3D 模型</sub></td>
  </tr>
</table>

<table>
  <summary>Altium PCB 库示例</summary>

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
</table>

<table>
  <summary>Altium SchLib 示例</summary>

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
</table>

## 参考和致谢

- 感谢[`easyeda2kicad`](https://github.com/uPesy/easyeda2kicad.py)@[uPesy](https://github.com/uPesy)给了我项目最开始的灵感，这是一个很优秀的工具，市面上`KiCad`相关的`easyeda`导入插件，绝大多数都是在这个工具的基础上进行开发的。
- 感谢群友菊猫赠送的一部分`TOKEN`，虽然也自费购买了一部分，但是最后算下来也属于半买半送了，那个时候`opus 4.6`还很好用，猛蹬`opus 4.6`，实现了很多功能，打下了好的基础。
- 所有帮助测试的群友，emoji老弟@[z217-cpu](https://github.com/z217-cpu)，🐏🐏@[Cloud-Lightning](https://github.com/Cloud-Lightning)等。
