<p align="center">
  <img src="assets/icons/seex.png" alt="SeEx Logo" width="160" />
</p>

<h1 align="center">SeEx</h1>

<p align="center">
  <strong>Seek &amp; Export</strong> - Clipboard Event Tracker
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-Ref42-0B3868" alt="license" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-66BB6A" alt="platform" />
</p>

---

## What It Does

SeEx monitors clipboard text, extracts LCSC component IDs, saves matched IDs to BOM CSV, and exports EDA libraries through [`npnp`](https://github.com/ref42/npnp).

The export page supports:

- Altium SchLib/PcbLib batch export
- KiCad symbol, footprint, and 3D model export
- merged Altium libraries
- parallel export jobs
- concise export status with elapsed time and failed IDs

## Download And Run

Release assets are built by GitHub Actions for Windows, macOS, and Linux. If you download a release archive, extract it and run the `seex` binary directly.

The release binary embeds the app icon, logos, and bundled font at compile time. The archive only needs the binary, `README.md`, and `LICENSE`.

Windows and macOS release binaries should run without installing build tools.

Linux release binaries may still need common desktop runtime libraries from the host distribution, especially X11/Wayland, GTK, DBus, and GPU/OpenGL related libraries. Most desktop Ubuntu/Fedora/Arch installs already have them. Minimal server images usually do not.

## Build Locally

Windows:

```bash
cargo build --release --locked --bin seex
```

macOS:

```bash
cargo build --release --locked --bin seex
```

Ubuntu/Debian build dependencies:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config \
  libx11-dev \
  libxi-dev \
  libxcb1-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxcursor-dev \
  libxrandr-dev \
  libxinerama-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libwayland-dev \
  libgl1-mesa-dev \
  libegl1-mesa-dev \
  libgtk-3-dev \
  libdbus-1-dev \
  libudev-dev
```

Then build:

```bash
cargo build --release --locked --bin seex
```

Fedora build dependencies:

```bash
sudo dnf install \
  pkgconf-pkg-config \
  libX11-devel \
  libXi-devel \
  libxcb-devel \
  libXcursor-devel \
  libXrandr-devel \
  libXinerama-devel \
  libxkbcommon-devel \
  libxkbcommon-x11-devel \
  wayland-devel \
  mesa-libGL-devel \
  mesa-libEGL-devel \
  gtk3-devel \
  dbus-devel \
  systemd-devel
```

Arch Linux build dependencies:

```bash
sudo pacman -S --needed \
  pkgconf \
  libx11 \
  libxi \
  libxcb \
  libxcursor \
  libxrandr \
  libxinerama \
  libxkbcommon \
  libxkbcommon-x11 \
  wayland \
  mesa \
  gtk3 \
  dbus \
  systemd-libs
```

## Release Builds

The release workflow is `.github/workflows/release.yml`.

It builds these targets:

- `x86_64-pc-windows-msvc` on `windows-2025-vs2026`
- `aarch64-pc-windows-msvc` on `windows-11-vs2026-arm`
- `x86_64-unknown-linux-gnu` on `ubuntu-26.04`
- `aarch64-unknown-linux-gnu` on `ubuntu-26.04-arm`
- Fedora `x86_64-unknown-linux-gnu` in `fedora:latest`
- Fedora `aarch64-unknown-linux-gnu` in `fedora:latest`
- Arch Linux `x86_64-unknown-linux-gnu` in `archlinux:latest`
- Arch Linux `aarch64-unknown-linux-gnu` in `archlinux:latest`
- `x86_64-apple-darwin` on `macos-26-intel`
- `aarch64-apple-darwin` on `macos-26`

The workflow installs Linux build packages because `eframe`/`egui`, clipboard access, and native file dialogs depend on platform GUI libraries. Ubuntu builds install packages with `apt`; Fedora and Arch builds run inside distro containers and install packages with `dnf`/`pacman`.

Some newest hosted runner labels, especially Windows ARM and Ubuntu 26.04, may be public preview on GitHub Actions. If GitHub changes runner availability, use the nearest stable label in `.github/workflows/release.yml`.

Create a release by pushing a version tag:

```bash
git tag -a v1.0.12 -m "SeEx v1.0.12"
git push origin v1.0.12
```

You can also run the workflow manually with `workflow_dispatch` and provide a tag name.

## License

See [LICENSE](LICENSE). Copyright (c) Ref42. All rights reserved.
