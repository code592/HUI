Unicode True
!include "MUI2.nsh"
!define MUI_ICON "hui.ico"
!define MUI_UNICON "hui.ico"
Name "HUI"
OutFile "HUI-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\HUI"
RequestExecutionLevel user
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
Section "HUI"
  SetOutPath "$INSTDIR"
  File "..\..\target\release\hui.exe"
  File "hui.ico"
  SetOutPath "$INSTDIR\assets\katex"
  File "..\..\vendor\katex\katex.min.css"
  File "..\..\vendor\katex\LICENSE"
  File /r "..\..\vendor\katex\fonts"
  WriteRegStr HKCU "Software\Classes\HUI.Markdown" "" "Markdown document"
  WriteRegStr HKCU "Software\Classes\HUI.Markdown\DefaultIcon" "" '"$INSTDIR\hui.ico"'
  WriteRegStr HKCU "Software\Classes\HUI.Markdown\shell\open\command" "" '"$INSTDIR\hui.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\.md\OpenWithProgids" "HUI.Markdown" ""
  WriteRegStr HKCU "Software\Classes\.markdown\OpenWithProgids" "HUI.Markdown" ""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HUI" "DisplayName" "HUI"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HUI" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\HUI.lnk" "$INSTDIR\hui.exe" "" "$INSTDIR\hui.ico"
SectionEnd
Section "Uninstall"
  DeleteRegValue HKCU "Software\Classes\.md\OpenWithProgids" "HUI.Markdown"
  DeleteRegValue HKCU "Software\Classes\.markdown\OpenWithProgids" "HUI.Markdown"
  DeleteRegKey HKCU "Software\Classes\HUI.Markdown"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HUI"
  Delete "$SMPROGRAMS\HUI.lnk"
  Delete "$INSTDIR\hui.exe"
  Delete "$INSTDIR\hui.ico"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /r "$INSTDIR\assets"
  RMDir "$INSTDIR"
SectionEnd
