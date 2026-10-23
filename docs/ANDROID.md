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
- 另存为 Markdown 后关联新 URI；HTML/PDF 导出只创建导出文件，不替换当前文档。
- 提供方不保证原子替换，写回中途磁盘/提供方错误仍可能使远端文件部分写入。私有镜像和恢复副本保留，不应把此路径宣称为与桌面文件系统同等的原子保存。
- 自动保存沿用桌面逻辑，进入后台时保存恢复快照。系统强杀、断电与各种第三方云盘提供方仍需额外故障注入验收。
- 文件入口目前限制 50 MiB；这只是导入上限，不是已达到大文档性能目标。

## 移动界面与边界

默认单栏阅读，提供源码编辑、触控滚动、文件/标签/大纲抽屉、排版和命令面板。九种 UI 语言复用桌面字典，系统语言从 Android Configuration 读取；文档保留 Unicode 原文。

仍待完成：文件夹树授权、文档相对图片资源批量导入、图片粘贴/插入、触控阅读选区手柄、完整软键盘/中文 IME 验收、无障碍阅读语义和真机能耗。通过单文件选择器导入的 Markdown 不会自动获得同目录图片权限；内嵌 data 图片可由已有原生引擎处理。部分平台错误诊断仍为英文。

源码编辑沿用有界段落窗口，不是完整连续虚拟编辑器。即时编辑、严格 HTML/CSS/公式/图表兼容与 50 MiB 首屏指标继承 [STATUS.md](STATUS.md) 中的未完成项。

## 集成测试

```sh
bash scripts/check-android-device.sh emulator-5560
```

需要已启动的 ARM64 模拟器。脚本只接受明确的 emulator 序列号，避免安装到连接的个人手机。单独测试 APK 中的 ContentProvider 验证实际导入后使用 Rust 规范路径写回、Unicode/CRLF 保真、重复保存、外部冲突、只读与权限撤销。测试 Provider 不进入产品 APK，测试结束卸载。

本次实际构建与验证结果见 [PLATFORM-VALIDATION.md](PLATFORM-VALIDATION.md)。
