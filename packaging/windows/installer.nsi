; GBOS Agent installer for Windows.
;
; Installs one file, GBOS-Agent.exe, and then runs it with --install-service to
; register and start the Windows service. Registering the service is left to the
; binary for the same reason the macOS package does it that way: the service's
; configuration has exactly one description, and a hand-run
; `GBOS-Agent.exe --install-service` produces the same result as this installer.
;
; Build:
;   makensis /DPRODUCT_VERSION=0.1.0 /DPRODUCT_VERSION_4=0.1.0.0 ^
;            /DSOURCE_EXE=<path to the built exe> installer.nsi
;
; Requires NSIS 3.x. The service runs as LocalSystem, which is not configurable
; here and must not become so: launching the per-session helper needs
; WTSQueryUserToken, which only LocalSystem may call.

Unicode true

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"

!ifndef PRODUCT_VERSION
    !define PRODUCT_VERSION "0.0.0"
!endif

; VIProductVersion requires four numeric components.
!ifndef PRODUCT_VERSION_4
    !define PRODUCT_VERSION_4 "0.0.0.0"
!endif

!ifndef SOURCE_EXE
    !define SOURCE_EXE "agent.exe"
!endif

!ifndef OUTFILE
    !define OUTFILE "GBOS-Agent-Windows.exe"
!endif

!define PRODUCT_NAME "GBOS Agent"
!define PRODUCT_PUBLISHER "GBaja"
!define EXE_NAME "GBOS-Agent.exe"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\GBOSAgent"

Name "${PRODUCT_NAME}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\GBOS"
InstallDirRegKey HKLM "Software\GBOS\Agent" "InstallDir"

; Registering a service and writing to Program Files both need elevation.
RequestExecutionLevel admin

SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${PRODUCT_VERSION_4}"
VIAddVersionKey "ProductName" "${PRODUCT_NAME}"
VIAddVersionKey "FileDescription" "${PRODUCT_NAME} installer"
VIAddVersionKey "FileVersion" "${PRODUCT_VERSION}"
VIAddVersionKey "ProductVersion" "${PRODUCT_VERSION}"
VIAddVersionKey "CompanyName" "${PRODUCT_PUBLISHER}"

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "GBOS is ready"
!define MUI_FINISHPAGE_TEXT "GBOS will connect automatically when this computer has internet access.$\r$\n$\r$\nOpen Devices in GBOS to see this computer. Setup is complete."
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
    ; Refuse to run twice at once: two copies would race over the same service
    ; registration and the same file. The name has no `Global\` prefix because a
    ; backslash in an NSIS string is an escape character, and a session-local
    ; name is enough — both copies would run in the same session. Fails open: if
    ; the error code cannot be read, the installer proceeds rather than blocking
    ; a legitimate install.
    System::Call 'kernel32::CreateMutex(p 0, i 0, t "GBOSAgentSetup") p .r1'
    System::Call 'kernel32::GetLastError() i .r2'

    ; ERROR_ALREADY_EXISTS. Compared as a string, which is both what LogicLib's
    ; `==` does and correct for a decimal error code.
    ${If} $2 == 183
        MessageBox MB_OK|MB_ICONEXCLAMATION "The ${PRODUCT_NAME} installer is already running."
        Abort
    ${EndIf}

    ${IfNot} ${RunningX64}
        MessageBox MB_OK|MB_ICONSTOP "${PRODUCT_NAME} requires 64-bit Windows."
        Abort
    ${EndIf}

    ; The uninstall key and the InstallDir key are both under the 64-bit view.
    SetRegView 64
    ; InstallDirRegKey uses the installer's default registry view. Read the
    ; saved 64-bit value explicitly so upgrades retain an existing custom path.
    ReadRegStr $0 HKLM "Software\GBOS\Agent" "InstallDir"
    ${If} $0 != ""
        StrCpy $INSTDIR $0
    ${EndIf}
FunctionEnd

Section "GBOS Agent" SecMain
    SectionIn RO

    ; Uninstall information, and the service, are per-machine.
    SetShellVarContext all
    SetRegView 64

    ; A previous version has to be deregistered before its file is replaced.
    ; Windows will not overwrite a running executable, and the service is
    ; holding this one open — so leaving it registered would make the copy below
    ; fail with a sharing violation on every upgrade.
    ${If} ${FileExists} "$INSTDIR\${EXE_NAME}"
        DetailPrint "Stopping the previous version..."
        nsExec::ExecToLog '"$INSTDIR\${EXE_NAME}" --uninstall-service'
        Pop $0

        ${If} $0 != 0
            MessageBox MB_OK|MB_ICONSTOP "GBOS could not stop the previous installation.$\r$\n$\r$\nRestart this computer and run this installer again."
            SetErrorLevel 1
            Abort
        ${EndIf}

        ; Service deletion can outlive its stop command. Wait for Windows to
        ; release the registration before replacing the executable.
        StrCpy $3 0
        ${Do}
            nsExec::ExecToStack '"$SYSDIR\sc.exe" query GBOSAgent'
            Pop $0
            Pop $1
            ${If} $0 == 1060
                ${ExitDo}
            ${EndIf}
            IntOp $3 $3 + 1
            ${If} $3 >= 40
                MessageBox MB_OK|MB_ICONSTOP "Windows is still stopping the previous GBOS installation.$\r$\n$\r$\nRestart this computer and run this installer again."
                SetErrorLevel 1
                Abort
            ${EndIf}
            Sleep 500
        ${Loop}
    ${EndIf}

    SetOutPath "$INSTDIR"
    File "/oname=${EXE_NAME}" "${SOURCE_EXE}"

    WriteUninstaller "$INSTDIR\uninstall.exe"

    DetailPrint "Registering the ${PRODUCT_NAME} service..."
    nsExec::ExecToLog '"$INSTDIR\${EXE_NAME}" --install-service'
    Pop $0

    ${If} $0 != 0
        MessageBox MB_OK|MB_ICONSTOP "GBOS could not finish setup (exit code $0).$\r$\n$\r$\nRestart this computer and run this installer again. If setup still fails, the details are in C:\ProgramData\GBOS\Agent\agent.log."
        SetErrorLevel 1
        Abort
    ${EndIf}

    WriteRegStr HKLM "Software\GBOS\Agent" "InstallDir" "$INSTDIR"

    WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT_NAME}"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${EXE_NAME}"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegStr HKLM "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'

    ; There is nothing to repair or modify: reinstalling is the repair.
    WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoRepair" 1

    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKLM "${UNINSTALL_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Uninstall"
    SetShellVarContext all
    SetRegView 64

    ; Deregister first: this stops the service, and the file cannot be deleted
    ; while the service has it open.
    ${If} ${FileExists} "$INSTDIR\${EXE_NAME}"
        DetailPrint "Stopping the ${PRODUCT_NAME} service..."
        nsExec::ExecToLog '"$INSTDIR\${EXE_NAME}" --uninstall-service'
        Pop $0

        Sleep 1500
    ${EndIf}

    Delete "$INSTDIR\${EXE_NAME}"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    DeleteRegKey HKLM "${UNINSTALL_KEY}"
    DeleteRegKey HKLM "Software\GBOS\Agent"

    ; C:\ProgramData\GBOS\Agent\agent.log is deliberately left behind. It is the
    ; only record of what the agent did, and is what someone would need if they
    ; are reinstalling to investigate a problem.
SectionEnd
