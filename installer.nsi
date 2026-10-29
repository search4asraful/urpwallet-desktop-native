!include "MUI2.nsh"
!include "FileFunc.nsh"

Name "urpwallet"
OutFile "target/installer/urpwallet-x86_64-setup.exe"
InstallDir "$PROGRAMFILES64\urpwallet"
InstallDirRegKey HKLM "Software\urpwallet" "Install_Dir"
RequestExecutionLevel admin

; Visual Styling
!define MUI_ABORTWARNING
!define MUI_ICON "assets/icon.ico"
!define MUI_UNICON "assets/icon.ico"

; Wizard Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\urpwallet.exe"
!insertmacro MUI_PAGE_FINISH

; Uninstaller Pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "urpwallet (required)" SecCore
    SectionIn RO

    SetOutPath "$INSTDIR"
    File "target/release/urpwallet.exe"
    File /nonfatal "assets/icon.ico"

    ; Write installation registry keys
    WriteRegStr HKLM "Software\urpwallet" "Install_Dir" "$INSTDIR"

    ; Windows Add/Remove Programs integration
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\urpwallet" "DisplayName" "urpwallet"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\urpwallet" "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\urpwallet" "DisplayIcon" '"$INSTDIR\urpwallet.exe"'
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\urpwallet" "Publisher" "urpwallet"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\urpwallet" "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\urpwallet" "NoRepair" 1

    ; Create uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; Start Menu Shortcut
    CreateDirectory "$SMPROGRAMS\urpwallet"
    CreateShortcut "$SMPROGRAMS\urpwallet\urpwallet.lnk" "$INSTDIR\urpwallet.exe" "" "$INSTDIR\urpwallet.exe" 0
    CreateShortcut "$SMPROGRAMS\urpwallet\Uninstall urpwallet.lnk" "$INSTDIR\uninstall.exe"

    ; Desktop Shortcut
    CreateShortcut "$DESKTOP\urpwallet.lnk" "$INSTDIR\urpwallet.exe" "" "$INSTDIR\urpwallet.exe" 0
SectionEnd

Section "Uninstall"
    Delete "$DESKTOP\urpwallet.lnk"
    Delete "$SMPROGRAMS\urpwallet\urpwallet.lnk"
    Delete "$SMPROGRAMS\urpwallet\Uninstall urpwallet.lnk"
    RMDir "$SMPROGRAMS\urpwallet"

    Delete "$INSTDIR\urpwallet.exe"
    Delete "$INSTDIR\icon.ico"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\urpwallet"
    DeleteRegKey HKLM "Software\urpwallet"
SectionEnd
