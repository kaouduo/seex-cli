<p align="center">
  <img src="assets/icons/seex.svg" alt="SeEx icon" width="200" />
</p>

<h1 align="center">SeEx (Seek and Export)</h1>

<p align="center">
  <strong>Batch export LCSC components in formats supported by Altium Designer (AD) and KiCad.</strong>
</p>

<p align="center"><a href="README.en.md">English</a> | <a href="README.md">简体中文</a></p>

---

## Before You Start

> [!NOTE]
> Although the source code is fully public, please follow the [LICENSE](LICENSE) when using this project.

The project history, in chronological order:

- The project began with `nlbn` (Nice Library Bring now, a pun on "拿来吧你"). It was open-sourced and received a number of stars, but was later uploaded to `Gitee` without permission. I stopped maintaining `nlbn` and realized that many people do not read the `README` or `License`, and have little respect for copyright or other people's work. Since the project originally only had an English README, which many people could not understand, `nlbn` is no longer maintained.
- While developing `nlbn`, I reverse-engineered parts of the `Altium Designer` file format and gradually created `npnp` (Normalize Pin Net Pad, a pun on "牛批牛批"). It was also open-sourced and copied. A corresponding `GUI` tool was then created using `tauri2`. Although it was rough, it helped many users solve their problems.
- That GUI tool became the current `seex` (Seek and Export). It originally called the `npnp` `cli` for exports, which meant maintaining two repositories. After the refactor, `seex` no longer depends on `npnp`; it contains the required functionality directly and uses the `egui/eframe` GUI framework. `npnp` may also continue to be maintained for users who need it in `CI` workflows.
- If you have a feature you would like to see, please contact me. Email: `lampmud@163.com` / `ybinlew@gmail.com`. You can also open an issue.
- Please do not rewrite this tool in `python`. Python is not my `taste`. Thank you for understanding.

## What Can SeEx Do?

### KiCad

- [x] Export KiCad-compatible `Symbol`, `Footprint`, and `3D` model formats.
- [x] Export these formats in batches or individually, including standalone `step` model exports.

### Altium Designer (AD)

- [x] Export individual `schlib` and `pcblib` libraries. `pcblib` includes the `step` model.
- [x] Support batch export.
- [x] Export multiple components into one merged `schlib` and `pcblib` library.

### Shared Features

- [x] Export metadata in Chinese or English.
- [x] Use multiple threads for downloads.
- [x] Add new components to existing libraries exported by `seex`.
- [x] Customize schematic stroke and fill colors.
- [x] Export `3D` models independently.

## Supported EDA Versions

> [!NOTE]
> These are the versions currently known to work. SeEx may support additional versions. If a library exported by SeEx cannot be opened by an older EDA version, please open an issue so compatibility can be considered.

- [x] `KiCad 9.0` to `Latest`
- [x] `Altium Designer 23.x` to `Latest`

## What Is Planned?

- [ ] Support more `EDA` tools, such as `allegro` and `pads`.
- [ ] Continue improving the current workflow and fix more hidden `bugs`.
- [ ] Keep improving interactions and usability, refine the icons, and try new design languages.
- [ ] Explore a broadly applicable way to remove or hide parts of `3D` models, such as specific markings.
- [ ] If projects need `CI` support, possibly restart and maintain the `npnp` `CLI` again.

## Who Would SeEx Like to Contribute?

- [x] People with software design experience who can help maintain the software over time.
- [x] People with good visual judgment who can provide valuable design feedback.
- [x] Designers who can help redesign the software icon and interface layout.
- [x] People who may have a better name for the software.

## What Will SeEx Not Support?

> [!NOTE]
> 1. SeEx will not add a `3D` model preview. Some derivative tools may include this feature, but I consider it unnecessary: preview it directly in the component store instead. Adding it to `seex` would take up interface space and memory.
> 2. The GUI framework will not be replaced in the short term unless a better framework becomes available.

### Export Examples

<table>
  <summary>KiCad examples</summary>
  <tr>
    <td align="center"><img src="imgs/kicad_symbol.png" alt="KiCad symbol" width="280" /><br /><sub>Symbol library</sub></td>
    <td align="center"><img src="imgs/kicad_footprint.png" alt="KiCad footprint" width="280" /><br /><sub>Footprint library</sub></td>
    <td align="center"><img src="imgs/kicad_3dmodel.png" alt="KiCad 3D model" width="280" /><br /><sub>3D model</sub></td>
  </tr>
</table>

<table>
  <summary>Altium PCB library examples</summary>

  <table>
    <tr>
      <td><img src="imgs/pcb_01.png" alt="Altium PCB example 1" width="420" /></td>
      <td><img src="imgs/pcb_02.png" alt="Altium PCB example 2" width="420" /></td>
      <td><img src="imgs/pcb_03.png" alt="Altium PCB example 3" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/pcb_04.png" alt="Altium PCB example 4" width="420" /></td>
      <td><img src="imgs/pcb_05.png" alt="Altium PCB example 5" width="420" /></td>
      <td><img src="imgs/pcb_06.png" alt="Altium PCB example 6" width="420" /></td>
    </tr>
  </table>
</table>

<table>
  <summary>Altium SchLib examples</summary>

  <table>
    <tr>
      <td><img src="imgs/sch_01.png" alt="Altium SchLib example 1" width="420" /></td>
      <td><img src="imgs/sch_02.png" alt="Altium SchLib example 2" width="420" /></td>
      <td><img src="imgs/sch_03.png" alt="Altium SchLib example 3" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/sch_04.png" alt="Altium SchLib example 4" width="420" /></td>
      <td><img src="imgs/sch_05.png" alt="Altium SchLib example 5" width="420" /></td>
      <td><img src="imgs/sch_06.png" alt="Altium SchLib example 6" width="420" /></td>
    </tr>
  </table>
</table>

## References And Thanks

- Thanks to [`easyeda2kicad`](https://github.com/uPesy/easyeda2kicad.py) and [uPesy](https://github.com/uPesy) for the project's original inspiration. It is an excellent tool, and most EasyEDA import plugins related to `KiCad` are based on it.
- Thanks to the community member 菊猫 for providing part of the `TOKEN`. I also paid for some of it, but the final result was roughly half purchased and half donated. `opus 4.6` was still very useful at the time, and it helped implement many features and establish a good foundation.
- Thanks to everyone who helped test the project, including emoji老弟 @[z217-cpu](https://github.com/z217-cpu), 🐏🐏 @[Cloud-Lightning](https://github.com/Cloud-Lightning), and others.
