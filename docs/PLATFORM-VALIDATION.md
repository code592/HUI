# 本次平台构建与验证

日期：2026-10-24。基于当前工作区源码，未自动发布 GitHub Release。

## 验证范围

- 核心测试：12 个通过，覆盖 Unicode/换行、事务、搜索和存储冲突。
- 九语言字典检查：9 × 138 个键、占位符与欢迎文档通过。
- Android ARM64：原生 Slint 入口及系统存储桥接；Android API 36 / 16 KiB 页模拟器上，通过原生初始化、Unicode/CRLF 保存、重复保存、外部冲突、只读和权限撤销测试。Content URI 打开与规范路径写回链路也已通过（包括进入 Rust 文档模型，覆盖 `/data/data` 和 `/data/user` 路径别名）；只读与撤权测试使用测试 Provider 注入错误，不是所有真实提供方的权限验收。
- Linux ARM64：已通过 Ubuntu 24.04 容器中多语言 PNG/PDF 导出、Xvfb 窗口截图及默认渲染器启动；共用手机宽度布局已用原生软件渲染截图检查。
- Linux x64：已完成本次优化构建及多语言 PNG/PDF、Xvfb 窗口、默认渲染器启动检查。
- Windows：GitHub Actions Windows Server 2022 原生 MSVC 环境已完成 x64/ARM64 构建；已完成 x64 原生 PNG/PDF 导出及窗口截图检查；补充应用私有 Noto CJK 字体后，简体中文缺字已修复并经渲染图复核。Mermaid 增强后的最终包正在重新构建。旧 ZIP 不作为本次交付。

- Mermaid：11 类常见图表（含 graph 别名）、中文与多行标签、深色缓存、错误语法及原生像素绘制测试已通过；共 16 个渲染模块测试通过。各端更新包的最终导出检查仍在进行，详见 [MERMAID.md](MERMAID.md)。

## 明确未验收

Windows 10 LTSC 2021、Windows 10 22H2/11 实机、Linux 桌面真实输入法与图形驱动、Android 10 最低版本及真机中文 IME、安全区/横竖屏/字体缩放完整矩阵、所有性能与能耗指标。Android 模拟器原生初始化测试不等同于完整触控 UI 验收。

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

Windows 的原生 MSVC 打包命令见 [DESKTOP-PACKAGING.md](DESKTOP-PACKAGING.md)。`.github/workflows/platform-packages.yml` 提供五个架构包的手动 CI 构建，上传工作流 artifacts，不自动发布 Release；Windows 两个架构的工作流已在 GitHub 执行成功；桌面 macOS、Windows、Linux 的测试、Clippy 与 Release 构建也已通过。
