# Windows / Linux 架构包

Windows 产物为含 MSVC 运行库的便携 ZIP，Linux 为 tar.gz（bin/share 布局）。两者均附 SHA-256 校验文件。它们不是已签名安装器。

## Windows

安装 Rust/MSVC、Visual Studio C++ 工具链、Windows SDK、CMake、Python 3（可通过 `PYTHON3` 指定解释器）；进入匹配目标的 Visual Studio Developer PowerShell。

```powershell
rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc
./scripts/package-windows.ps1 -Arch x64
./scripts/package-windows.ps1 -Arch arm64
```

ARM64 构建需要 VS 的 ARM64 C++ 库与运行库。脚本从 `VCToolsRedistDir` 复制目标架构 CRT 到便携包。解压后运行 hui.exe；可通过系统“打开方式”关联 Markdown，便携包不自动更改关联。

## Linux

在对应架构的 Ubuntu 22.04 中安装 Rust 1.98、clang、cmake、ninja-build、pkg-config、libfontconfig1-dev、libxkbcommon-dev、libxkbcommon-x11-0、libxcb-shape0-dev、libxcb-xfixes0-dev、libudev-dev、libssl-dev、libgl1-mesa-dev、python3、curl、git。

```sh
bash scripts/package-linux.sh
bash scripts/check-linux-package.sh arm64 # x64 环境改为 x64
```

可设置 `CARGO_TARGET_DIR` 与 `HUI_BUILD_JOBS`。打包脚本拒绝直接在 macOS 执行，防止把 macOS 二进制标记成 Linux 包。

Linux 解压后执行 bin/hui。Ubuntu 运行依赖：`sudo apt install libfontconfig1 libxkbcommon0 libxkbcommon-x11-0 libxcb-shape0 libxcb-xfixes0 libudev1 libgl1 libegl1 fonts-noto-cjk`。需要正常的 OpenGL/X11 或 Wayland 桌面环境。将 bin 和 share 安装到同一前缀后，可由桌面系统发现 .desktop 与统一 Logo。

编译和 CLI 渲染检查不能代替 Windows 10 LTSC 2021、Linux 桌面、中文输入法和图形驱动的实机验收。

四个包齐全后执行 `python3 scripts/check-desktop-archives.py`，验证 EXE/ELF 和 CRT 架构、必需资源及 SHA-256。只检查单个包可传入例如 `HUI-linux-arm64`。

Windows GUI 冒烟测试应在已登录用户的交互桌面会话执行；SYSTEM 服务会话的 OpenGL 行为不能代表实际桌面。运行检查应解压 ZIP 后使用包内 exe，并移除开发工具目录 PATH，验证运行库确实来自包或系统。

Windows 解压运行检查：`./scripts/check-windows-package.ps1 -Arch x64`（ARM64 改为 arm64）。输出位于 `.build/cross/smoke-windows-<架构>`，使用独立临时配置，不修改用户文档。
