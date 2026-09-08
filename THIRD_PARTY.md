# Third-party software

HUI's own source code is licensed under MIT. That license does not replace the licenses of dependencies, fonts or bundled resources.

The desktop application integrates Slint 1.17.1 under the **Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0**. Its complete text is in [vendor/slint/LICENSE.md](vendor/slint/LICENSE.md). The Slint attribution badge is displayed in every README and on the public release page. This applies to general-purpose desktop applications; it does not grant an embedded-system license.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

Key bundled components:

| Component | Use | License / source |
| --- | --- | --- |
| Slint 1.17.1 | Native UI | Royalty-free 2.0, see above; [source](https://github.com/slint-ui/slint) |
| Blitz 0.3.0-beta.2 | Native HTML/CSS layout | MIT OR Apache-2.0; [source](https://github.com/DioxusLabs/blitz) |
| Skia / skia-safe 0.99.0 | Native painting and PDF | BSD-style / MIT; [Skia](https://skia.org), [rust-skia](https://github.com/rust-skia/rust-skia) |
| KaTeX 0.16.22 | Formula validation, exported CSS/fonts | [MIT license](vendor/katex/LICENSE) and upstream distribution; [source](https://github.com/KaTeX/KaTeX) |
| MathJax 3.2.2 | Formula SVG output | [Apache-2.0](vendor/mathjax/LICENSE); [source](https://github.com/mathjax/MathJax) |
| merman 0.8.0-alpha.6 | Native diagram rendering | MIT OR Apache-2.0; [source](https://github.com/Latias94/merman) |
| MSVC runtime DLLs (Windows packages) | C/C++ runtime | Microsoft Visual Studio redistributable terms; not covered by HUI's MIT license |

`Cargo.lock` records exact Rust dependencies and checksums. Review each dependency's upstream license before redistributing a modified build. The list above is an overview, not a replacement for the license files. Scripts package the licenses for directly vendored assets with the application. System fonts are used from the operating system and are not redistributed by HUI. Linux users can install the separately licensed Noto CJK system package.
