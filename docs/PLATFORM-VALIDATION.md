# 本次平台构建与验证

日期：2026-10-24。本批应用源码提交为 `dbb1da7fb2c06ca9201bfd1f15e656e9b3f737ce`；后续仅补充测试资源路径与验证文档。未发布 GitHub Release。

## 验证范围

- 核心测试：12 个通过，覆盖 Unicode/换行、事务、搜索和存储冲突。
- 九语言字典检查：9 × 138 个键、占位符与欢迎文档通过。
- Android ARM64：原生 Slint 入口及系统存储桥接；Android API 36 / 16 KiB 页模拟器上，通过原生初始化、Unicode/CRLF 保存、重复保存、外部冲突、只读和权限撤销测试。Content URI 打开与规范路径写回链路也已通过（包括进入 Rust 文档模型，覆盖 `/data/data` 和 `/data/user` 路径别名）；只读与撤权测试使用测试 Provider 注入错误，不是所有真实提供方的权限验收。最终 APK 的 v2 签名与 16 KiB ELF/APK 对齐已检查；另在同一模拟器执行了 5 项 Mermaid ARM64 原生测试，覆盖 11 类图表的矢量输出、原生像素绘制、中文标签、深色背景及错误语法，全部通过。
- Linux ARM64：最终包已通过 Ubuntu 24.04 容器中多语言 PNG/PDF、Mermaid PNG/HTML/PDF 与深色 PNG 导出、Xvfb 窗口截图及默认渲染器启动；共用手机宽度布局已用原生软件渲染截图检查。
- Linux x64：最终包已完成本次优化构建及多语言 PNG/PDF、Mermaid PNG/HTML/PDF 与深色 PNG、Xvfb 窗口、默认渲染器启动检查。两个 Linux 包均在 Ubuntu 22.04 构建，在 Ubuntu 24.04 容器运行；原生图表 PDF 未包含光栅图像对象。
- Windows：最终 x64/ARM64 包已在 GitHub Actions Windows Server 2022 原生 MSVC 环境构建成功。x64 已完成多语言 PNG/PDF、Mermaid PNG/HTML/PDF、深色图表和 Slint 窗口截图检查；中文标签、深色背景与界面经过图像复核，图表 PDF 无光栅图像对象。应用私有 Noto CJK 字体、两种架构的 PE/DLL、九语言资源与 SHA-256 检查通过。ARM64 仅完成构建及包结构检查，未运行验收。

- Mermaid：11 类常见图表（含 graph 别名）、中文与多行标签、深色缓存、错误语法及原生像素绘制测试已通过；共 17 个渲染模块测试通过（含深色背景像素回归）。桌面更新包的最终导出检查及 Android 原生图表测试均已通过；这不是全部 Mermaid 语法的严格兼容认证，详见 [MERMAID.md](MERMAID.md)。

## 明确未验收

Windows 10 LTSC 2021、Windows 10 22H2/11 实机、Linux 桌面真实输入法与图形驱动、Android 10 最低版本及真机中文 IME、安全区/横竖屏/字体缩放完整矩阵、所有性能与能耗指标。Android 模拟器原生初始化和图表绘制测试不等同于完整触控 UI 验收。

Android 为开发签名预览；正式商店签名、iOS 实现与严格 Markdown/公式/图表兼容不在本次完成范围内。当前架构的其余限制继续见 [STATUS.md](STATUS.md) 和 [ANDROID.md](ANDROID.md)。

## 复现入口

```sh
cargo test -p hui-core --locked
python3 scripts/check-locales.py
bash scripts/package-linux.sh
python3 scripts/check-desktop-archives.py HUI-linux-arm64
bash scripts/package-android.sh preview
bash scripts/check-android-device.sh emulator-5560
```

Windows 的原生 MSVC 打包命令见 [DESKTOP-PACKAGING.md](DESKTOP-PACKAGING.md)。`.github/workflows/platform-packages.yml` 提供五个架构包的手动 CI 构建，上传工作流 artifacts，不自动发布 Release；Windows 两个架构的[打包工作流](https://github.com/code592/HUI/actions/runs/39992720810)已执行成功；三桌面的工作区测试与 Clippy 已通过，Linux/macOS 的独立 Release 检查及 Windows 打包中的 Release 构建已通过。[完整桌面 CI](https://github.com/code592/HUI/actions/runs/39992720830)保留各步骤记录。

## Android 原生图表复现

安装 ARM64 Android Rust 目标并配置 SDK/NDK 链接器后：

```sh
cargo test -p hui-render --release --target aarch64-linux-android --lib --no-run --locked
# 将 Cargo 输出的 hui_render-* 可执行文件推送至专用模拟器：
adb -s emulator-5560 push <test-binary> /data/local/tmp/hui-render-tests
adb -s emulator-5560 shell chmod 755 /data/local/tmp/hui-render-tests
adb -s emulator-5560 shell 'HUI_RENDER_TEST_ASSETS=/data/local/tmp HUI_RENDER_TEST_OUTPUT=/data/local/tmp/hui-diagram-output /data/local/tmp/hui-render-tests diagrams::tests:: --nocapture'
```

这些图表测试仅使用内嵌 SVG；`HUI_RENDER_TEST_ASSETS` 指向存在的测试目录，避免使用编译机的资源路径。`HUI_RENDER_TEST_OUTPUT` 可选，用于保存各图表的原生排版 PNG。

## Android 触控选择和 TXT（2026-11-03，版本码 5）

- 11 项核心测试、21 项渲染测试通过；包含多语言词/字素边界、段落空隙拖选、两种缩放下的手柄位置与滚动、全文选择及 TXT 输出。
- 工作区 Clippy、Android 目标 Clippy 均以 `-D warnings` 通过；九种语言各 141 个键通过目录校验。
- API 36 / ARM64 / 16 KiB 模拟器内，Instrumentation 驱动真实 Slint 窗口指针事件，验证阅读和编辑长按、手柄跨段/跨行拖动及 Unicode 复制。
- 大于初始 8192 字符窗口的测试文档验证全文复制、光标导航取消全选、全选后替换与撤销，不丢失屏外原文。
- TXT 经过实际导出工作线程、`text/plain` 系统创建文档 Intent 和隔离测试 Provider 回调，验证尾部标记及中文、emoji、组合字符。
- 原有原生初始化、字体/安全区、URI 导入、规范路径写回、Unicode/CRLF、外部冲突、只读与权限撤销测试仍通过。阅读/源码选区 PNG 由应用自身排版绘制后导出，并非系统屏幕截图。

此项不代表 Android 10、实体设备、全部输入法或连续跨源码窗口拖选已完成验收。普通拖选仍受当前源码窗口/即时块边界限制，全文全选和 TXT 导出覆盖整份文档。

## macOS TXT 更新包（2026-11-03，0.2.0 构建 4）

- Apple Silicon ARM64 Release 应用与 DMG 已重新构建，包含 UTF-8 TXT 导出、选区修复和现有 Mermaid 支持。
- 原生“文件 → 导出 TXT…”保存面板实际导出成功；12,618 字符、618 行测试文档包含九种语言、emoji、组合字符和屏外尾部标记，与命令行导出逐字节一致。
- 命令行 `.txt` / `.TXT` 输出一致；TXT 不依赖公式或图表排版，即使 Mermaid 语法错误也保留其代码文本。菜单导出使用显式格式，避免被文件名后缀改变。
- `cargo check`、应用全部目标 Clippy（`-D warnings`）、九语言 141 键检查通过；最终应用的 Mermaid HTML/PDF 导出与原生源码选区快照通过。
- 严格签名、Info.plist、图标、Markdown 文件关联及 DMG 校验通过。只读挂载后的可执行文件与交付应用一致，Applications 链接正确；测试副本及挂载已关闭。

本包由当前未提交工作区构建，来源及校验值见 `dist/BUILD-INFO.json`。部署目标为 macOS 12.0，最低版本与 Intel 未实测；使用本地临时签名，未做 Apple 公证。Windows/Linux 和 Android 安装包未在本次 macOS 打包中重新生成。
