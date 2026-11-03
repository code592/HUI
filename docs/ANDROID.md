# Android 原生应用

HUI 的 Android 宿主复用 `hui-core`、`hui-render` 和 Slint 界面，不含 WebView。最低 Android 10（API 29），当前打包 ABI 为 ARM64。Android 是开发预览，未完成桌面功能的全部移动验收。

## 构建

需要 Rust 1.98.0、JDK 17+、Python 3、libclang、Android SDK Platform 36、Build Tools 36.0.0、NDK 28.2.13676358。首次构建需要联网；Gradle Wrapper 固定为 9.3.1，AGP 为 9.1.0。

```sh
export ANDROID_HOME=/path/to/Android/sdk
export JAVA_HOME=/path/to/jdk
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/28.2.13676358"
bash scripts/package-android.sh preview
```

输出 `dist/HUI-android-arm64-preview.apk` 及 SHA-256。Preview 使用本机 Android 开发证书和独立应用 ID `dev.hui.markdown.preview`，属于可侧载的测试包，不是商店发行签名。重新在另一台机器构建时开发证书不同，不能直接覆盖原安装。

`bash scripts/package-android.sh release` 生成未签名 Release APK，正式分发前需要自行使用开发者密钥签名。不要把密钥或口令提交到仓库。

原生库使用 16 KiB ELF 页对齐。`scripts/check-android-package.py` 检查 ABI、对齐、启动图标、离线数学字体和许可证。该检查不替代 Android 真机兼容性、能耗或性能测试。

## 文件与生命周期

- 使用系统文档选择器打开，支持系统“打开方式”和分享文本/文档；默认进入阅读模式。
- 系统授予的 `content://` URI 在 Java 层处理；Rust 使用应用私有目录的 UTF-8 文档镜像，URI 映射统一使用规范路径，避免 `/data/data` 与 `/data/user` 别名丢失写回关联。应用不申请全盘存储权限。
- 保存先原子写入私有镜像，再写回文档提供方。写回前比较 SHA-256，发现外部修改时拒绝覆盖；本地编辑继续保留，可另存为。
- 另存为 Markdown 后关联新 URI；HTML/PDF/TXT 导出只创建导出文件，不替换当前文档。TXT 使用 UTF-8，通过系统文档选择器保存为 `text/plain`，保留正文、段落、代码、公式源码与图片替代文字，去掉 Markdown 排版标记，不需要执行公式或图表渲染。
- 提供方不保证原子替换，写回中途磁盘/提供方错误仍可能使远端文件部分写入。私有镜像和恢复副本保留，不应把此路径宣称为与桌面文件系统同等的原子保存。
- 自动保存沿用桌面逻辑，进入后台时保存恢复快照。系统强杀、断电与各种第三方云盘提供方仍需额外故障注入验收。
- 文件入口目前限制 50 MiB；这只是导入上限，不是已达到大文档性能目标。

## 移动界面与边界

默认单栏阅读，提供源码编辑、触控滚动、文件/标签/大纲抽屉、排版和命令面板。九种 UI 语言复用桌面字典，系统语言从 Android Configuration 读取；文档保留 Unicode 原文。

阅读与源码编辑支持长按选词、拖动两个选区手柄，以及浮动复制/全选操作栏；编辑时提供剪切和粘贴。450 毫秒长按期间移动超过 8 逻辑像素转为滚动，避免滑动阅读误选。拖动手柄到边缘时按需滚动，释放后停止定时器。选词遵循 Unicode 词边界，emoji 与组合字符按字素保留；汉字默认单字起选，可拖动扩展。

阅读全选覆盖完整原生排版文档；源码全选复制读取完整 Rope，不受 8192 字符编辑窗口限制。普通手柄拖动仍以当前已加载的源码窗口为边界，即时编辑中的选择以当前块为边界；这些能力不等于完成连续虚拟编辑器。

仍待完成：文件夹树授权、文档相对图片资源批量导入、图片粘贴/插入、完整软键盘/中文 IME 验收、无障碍阅读语义和真机能耗。通过单文件选择器导入的 Markdown 不会自动获得同目录图片权限；内嵌 data 图片可由已有原生引擎处理。部分平台错误诊断仍为英文。

源码编辑沿用有界段落窗口，不是完整连续虚拟编辑器。即时编辑、严格 HTML/CSS/公式/图表兼容与 50 MiB 首屏指标继承 [STATUS.md](STATUS.md) 中的未完成项。

## 集成测试

```sh
bash scripts/check-android-device.sh emulator-5560
```

需要已启动的 ARM64 模拟器。脚本只接受明确的 emulator 序列号，避免安装到连接的个人手机。单独测试 APK 中的 ContentProvider 验证实际导入后使用 Rust 规范路径写回、Unicode/CRLF 保真、重复保存、外部冲突、只读与权限撤销。测试 Provider 不进入产品 APK，测试结束卸载。

本次实际构建与验证结果见 [PLATFORM-VALIDATION.md](PLATFORM-VALIDATION.md)。

## 2026-11-03: CJK fonts and safe content area

Android UI, source, reading and diagram rendering now register the bundled Noto Sans CJK SC font (OFL 1.1). CJK script fallback uses this font, and Android body text prefers it. The font bytes are shared across registries and cached; no system font installation is required. Embedding the font increases the APK size.

All content, drawers, notices and settings are placed within Slint dynamic system safe-area insets, with keyboard bounds limiting usable height. The API 36 test emulator reported top 51.8 and bottom 24 logical pixels; these are system values, not device-specific constants.

Version code is now 4, using the same development signing certificate. Native instrumentation checks actual insets, font registration and an app-rendered snapshot. Chinese glyph coverage, the native image, and simulated safe-area reading/settings layouts have been checked. Full physical-device, landscape-cutout and IME acceptance remain pending.

## 2026-11-03：触控选择与 TXT 导出（版本码 5）

阅读和源码的选区使用各自实际绘制的排版坐标。手柄落在段落空隙时就近定位文字，不冻结选区。源码最左/右侧的手柄触摸区域保持在视口内；轻点源码通过原生 TextInput 重新启用输入法，仍未替代完整 IME 组合输入验收。

API 36 / ARM64 / 16 KiB 模拟器中的应用内指针事件测试覆盖长按、跨段/跨行手柄拖动、Unicode 剪贴板、超过 8192 字符窗口的全文全选、取消全选后定位光标、全文替换及撤销。TXT 测试拦截 `ACTION_CREATE_DOCUMENT` 的 `text/plain` Intent，将结果写入隔离 Provider，并验证文档末尾和 Unicode 内容。阅读与编辑选区的应用自身绘制 PNG 已检查。

复现仍使用 `scripts/check-android-device.sh emulator-5560`；新增图像重组脚本将测试输出保存到 `.build/android-touch-images/`。产品没有新增持续运行的测试任务或输入模拟；私有测试入口不向 Intent、文档或 UI 开放。
