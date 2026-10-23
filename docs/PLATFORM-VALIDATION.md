# 本次平台构建与验证

日期：2026-10-23。基于当前工作区源码，未自动发布 GitHub Release。

## 验证范围

- 核心测试：12 个通过，覆盖 Unicode/换行、事务、搜索和存储冲突。
- 九语言字典检查：9 × 138 个键、占位符与欢迎文档通过。
- Android ARM64：原生 Slint 入口及系统存储桥接；Android API 36 / 16 KiB 页模拟器上，通过原生初始化、Unicode/CRLF 保存、重复保存、外部冲突、只读和权限撤销测试。Content URI 打开追加回归与最终包复测正在进行。
- Linux ARM64：已通过 Ubuntu 24.04 容器中多语言 PNG/PDF 导出、Xvfb 窗口截图及默认渲染器启动；最终共用界面调整正在重新构建。
- Linux x64 / Windows：最终构建进行中。不能将 dist 中历史文件误当成本次产物。

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

Windows 的原生 MSVC 打包命令见 [DESKTOP-PACKAGING.md](DESKTOP-PACKAGING.md)。`.github/workflows/platform-packages.yml` 提供五个架构包的手动 CI 构建，上传工作流 artifacts，不自动发布 Release；新增工作流尚未在 GitHub 执行。
