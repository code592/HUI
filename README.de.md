# HUI

[简体中文](README.md) · [繁體中文](README.zh-Hant.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [Español](README.es.md)

<img src="assets/brand/hui.svg" width="72" height="72" alt="HUI">

Ein moderner, übersichtlicher Markdown-Editor und -Reader mit lokaler Datenhaltung, entwickelt mit **Rust + Slint + Blitz/Skia**, ohne WebView.

![HUI de](docs/screenshots/i18n-de.png)

## Status

**Entwicklungsvorschau 0.2.0**. Desktop-Pakete sind verfügbar. Eine native Android-ARM64-App und der APK-Build sind jetzt enthalten; die mobile Umsetzung hat noch Einschränkungen. iOS ist noch nicht verfügbar. Siehe [Android-Build und Einschränkungen](docs/ANDROID.md) sowie [Plattformprüfung](docs/PLATFORM-VALIDATION.md).

## Funktionen

- Quelltext, geteilte Vorschau, Lesen und experimentelle Direktbearbeitung; proportionale Scroll-Synchronisierung in beide Richtungen.
- Oberfläche auf vereinfachtem und traditionellem Chinesisch, Englisch, Japanisch, Koreanisch, Französisch, Deutsch, Russisch und Spanisch. Standardmäßig gilt die Systemsprache; manuelle Auswahl unter Typografie.
- Mehrsprachiges UTF-8-Markdown, Syntaxhervorhebung, Tabellen, Aufgabenlisten, lokale Bilder, Formeln und native Mermaid-Diagramme.
- Helles, dunkles und Papierdesign; eigene Quelltextgröße sowie einstellbare Zeilenhöhe, Absatzabstände und Lesebreite.
- Lokale Dateien, Tabs, zuletzt geöffnete Dateien, Suchen/Ersetzen, Rückgängig/Wiederholen, automatisches Speichern nach Schreibpausen, Konfliktschutz und Wiederherstellung.
- Auswahl und Kopieren im Lesemodus, Kontextmenüs, Markdown-Tastenkürzel und HTML/PDF-Export.

## Herunterladen und starten

Wählen Sie Ihre Architektur unter [Releases](https://github.com/code592/HUI/releases). macOS: DMG öffnen und HUI nach Applications ziehen. Windows: die gesamte ZIP-Datei entpacken und hui.exe starten; DLLs und assets daneben belassen. Linux: die unten genannten Abhängigkeiten installieren, tar.gz entpacken und ./bin/hui starten. Die Pakete sind nicht offiziell signiert oder notarisiert.

Die Android- und Mermaid-Änderungen dieses Branches sind Entwicklungsvorschauen und noch nicht unter Releases veröffentlicht. Windows-Artefakte gibt es im [Paket-Workflow](https://github.com/code592/HUI/actions/workflows/platform-packages.yml); die folgenden Skripte erstellen Linux-/Android-Pakete in `dist/`.

## Plattformen

Das macOS-Paket ist für Apple Silicon; Windows und Linux bieten x64/ARM64, Android eine ARM64-Vorschau. Diese Pakete werden in Windows-Server-2022-CI, Linux-Containern und einem Android-API-36-Emulator geprüft; für Windows ARM64 ist nur der Build geprüft. Frühere Ergebnisse einer Windows-11-ARM-VM bestätigen nicht die Abnahme dieser Pakete. Windows 10 LTSC 2021 / 22H2, macOS 12/Intel, die Mindestversion Android 10 und die vollständige Prüfung auf physischen Geräten stehen noch aus. Siehe [Prüfbericht](docs/PLATFORM-VALIDATION.md).

## Sprachen und Schriften

Die Systemsprache wird beim Start gelesen. Nicht unterstützte Sprachen fallen auf Englisch zurück. Chinesische Schrift- und Regionsangaben unterscheiden vereinfachtes und traditionelles Chinesisch. Die manuelle Auswahl bleibt gespeichert. Ein Sprachwechsel übersetzt oder verändert keine Dokumenttexte. CJK-Zeichen nutzen Ersatzschriften des Systems; auf minimalen Linux-Systemen Noto CJK installieren. Native Dialogschaltflächen stammen vom Betriebssystem.

## Kompilieren

Erforderlich sind Rust 1.98+, eine C/C++-Werkzeugkette, CMake, Python 3 und die Entwicklungsbibliotheken der Plattform. Windows benötigt die C++-Werkzeuge von Visual Studio 2022 und das passende SDK; mit PYTHON3 lässt sich der Interpreter festlegen. Beim ersten Build werden Abhängigkeiten und Skia-Binärdateien heruntergeladen.

```sh
cargo run -p hui-app -- path/to/document.md
cargo test --workspace --locked
python3 scripts/check-locales.py
```

Linux (Ubuntu 22.04 / 24.04):

```sh
sudo apt install build-essential clang cmake pkg-config python3 libfontconfig1-dev libxkbcommon-dev libxkbcommon-x11-0 libxcb-shape0-dev libxcb-xfixes0-dev libudev-dev libssl-dev libgl1 libegl1 fonts-noto-cjk
bash scripts/package-linux.sh
```

macOS:

```sh
HUI_DMG=1 bash scripts/package-macos.sh
```

Windows (Visual Studio Developer PowerShell):

```powershell
./scripts/package-windows.ps1 -Arch x64
./scripts/package-windows.ps1 -Arch arm64
```

## Bekannte Einschränkungen

Der Quelltexteditor nutzt eine begrenzte, seitenweise Ansicht; die Direktbearbeitung ist eine experimentelle Einzelblock-Implementierung. Durchgehende virtuelle Bearbeitung, mehrere Cursor und plattformübergreifende IME-Abnahme fehlen noch. Die Ziele für den ersten Bildaufbau bei 50 MB, Eingabe-/Scroll-Latenz und Energieverbrauch sind nicht durchgängig geprüft. Blitz HTML/CSS und merman wurden noch nicht streng mit offiziellen Ausgaben verglichen. Formeln nutzen KaTeX 0.16.22 zur Prüfung und MathJax 3.2.2 zur SVG-Anzeige, ohne strikte KaTeX-Gleichheit. PDF-Formeln sind Vektorpfade; Link-Anmerkungen, komplexe Seitenteilung und vollständig eingebettete HTML-Ressourcen bleiben unvollständig. Externe Ressourcen sind im nativen Lesemodus deaktiviert.

Beim Kopieren von PDF-Text können einzelne CJK- und kyrillische Zeichen durch optisch gleiche Unicode-Zeichen ersetzt werden; exakte Zeichentreue im exportierten PDF ist nicht garantiert.

## Beiträge und Lizenzen

Der eigene HUI-Quellcode steht unter [MIT](LICENSE). Abhängigkeiten, Schriften und Ressourcen behalten ihre jeweiligen Lizenzen; siehe [THIRD_PARTY.md](THIRD_PARTY.md). Fehlermeldungen sollten Plattform, Architektur, Version und ein minimales Beispieldokument ohne private Inhalte enthalten. Details zu offenen Punkten stehen in [docs/STATUS.md](docs/STATUS.md) (Chinesisch).

Mermaid wird für 11 gängige Diagrammtypen, CJK- und mehrzeilige Beschriftungen sowie dunkle Designs getestet. Siehe [Nutzung und Kompatibilitätsgrenzen](docs/MERMAID.md). Windows-Pakete enthalten eine private CJK-Ersatzschrift.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

```sh
# Android ARM64 — ANDROID_HOME / JAVA_HOME required
bash scripts/package-android.sh preview
```
