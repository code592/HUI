# HUI 统一应用标识

矢量母版：`assets/brand/hui.svg`。图形将字母 H 与打开的两页书合为一体，蓝色 `#477AD9` 与应用主色一致，白色书页在小尺寸下保持识别。没有文字缩写、阴影或细碎装饰。

`hui-mark.svg` 是透明背景单色标记；`hui-square.svg` 是移动端不预裁圆角的满版底图。所有平台由同一母版生成，不独立修改派生图形。

| 平台 | 资源与接入 | 验证状态 |
| --- | --- | --- |
| macOS | `packaging/macos/HUI.icns`；Info.plist 的 CFBundleIconFile 与打包脚本已接入 | ICNS 解码、iconutil 反向解包、当前 macOS 构建及 Finder 实际图标显示验证 |
| Windows | `packaging/windows/hui.ico`；Rust 构建脚本嵌入 EXE；NSIS 安装器、快捷方式、文档打开方式使用同一图标 | ICO 16/24/32/48/64/128/256 尺寸验证；Windows x64/ARM64 已编译并进行窗口检查；安装器验收未完成 |
| Linux | `packaging/linux/icons/hicolor` 与 desktop 文件的 `Icon=hui`；`scripts/package-linux.sh` 打包 bin/share | PNG/SVG 资源验证；Linux 桌面实机未验证 |
| Android | `packaging/android/res`：密度位图、v26 自适应层、v33 单色层；`AndroidManifest.icons.xml` 是合并片段 | XML 与资源引用检查；宿主工程和真机接入未完成 |
| iOS | `packaging/ios/Assets.xcassets/AppIcon.appiconset`；`Icons.xcconfig` 设置 AppIcon 名称 | 所有 catalog 尺寸和 RGB 无 alpha 校验；宿主工程和真机接入未完成 |

桌面窗口使用同一份 `assets/brand/hui-256.png`。应用内不额外增加常驻品牌占位，不改变编辑器交互布局。

## 平台适配

- macOS 保留外围透明留白；iOS 使用不预裁圆角、无透明通道的方形资源，由系统处理外形。
- Android 前景围绕中心缩放至 90%，保持在 108dp 画布中央的 66dp 安全圆内；圆形、圆角矩形以及单色主题共用原始书页路径。
- Android 宿主将 res 合并到 `app/src/main/res`，将图标属性合并到已有 application 标签。不要用片段替换完整 manifest。v33 单色配置需要 compileSdk 33+，不改变 Android 10 的最低目标。
- iOS 宿主将 Assets.xcassets 加入目标资源，合入 Icons.xcconfig（或设置 `ASSETCATALOG_COMPILER_APPICON_NAME=AppIcon`）。包含 iPhone、iPad 与 1024px 营销图标。
- Windows 需要 Windows SDK 的 rc.exe；交叉编译可用 RC 指定匹配目标的资源编译器。GNU 目标使用 windres。
- Linux 将打包后的 bin 与 share 安装到同一前缀（例如 ~/.local），再刷新桌面与图标缓存。安装器不修改默认 Markdown 应用。

平台规则参考：[Apple AppIcon 配置](https://developer.apple.com/documentation/xcode/configuring-your-app-icon)、[Android 自适应图标](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive)。

## 再生成

生成器只在制作资源时需要 Node.js 与 `sharp@0.35.4`，应用运行和常规构建不依赖它们。

```sh
npm install --prefix .build/icon-tools sharp@0.35.4
NODE_PATH="$PWD/.build/icon-tools/node_modules" node scripts/generate-icons.cjs
```

检查 `docs/screenshots/logo-preview.png` 的大图、平台留白与 16–96px 小图。重新生成资源后按平台重新打包；macOS 已运行的旧进程需保存文档、退出并重新打开应用。

macOS 图标更新需递增 CFBundleVersion；打包脚本在签名后更新应用包目录时间，避免 Finder 沿用旧图标缓存。2026-09-07 将构建号更新为 2，并针对 HUI 刷新 LaunchServices 注册后，已在 Finder 确认显示蓝色 H 图标。
