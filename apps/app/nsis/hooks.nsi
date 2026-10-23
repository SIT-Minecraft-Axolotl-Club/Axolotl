; https://nsis.sourceforge.io/ShellExecWait
!define AXL_INSTALLER_UI_PATH "${__FILEDIR__}\..\..\..\target\release\axolotl-installer-ui.exe"

!macro ShellExecWait verb app param workdir show exitoutvar ;only app and show must be != "", every thing else is optional
    #define SEE_MASK_NOCLOSEPROCESS 0x40
    System::Store S
    !if "${NSIS_PTR_SIZE}" > 4
    !define /ReDef /math SYSSIZEOF_SHELLEXECUTEINFO 14 * ${NSIS_PTR_SIZE}
    !else ifndef SYSSIZEOF_SHELLEXECUTEINFO
    !define SYSSIZEOF_SHELLEXECUTEINFO 60
    !endif
    System::Call '*(&i${SYSSIZEOF_SHELLEXECUTEINFO})i.r0'
    System::Call '*$0(i ${SYSSIZEOF_SHELLEXECUTEINFO},i 0x40,p $hwndparent,t "${verb}",t $\'${app}$\',t $\'${param}$\',t "${workdir}",i ${show})p.r0'
    System::Call 'shell32::ShellExecuteEx(t)(pr0)i.r1 ?e' ; (t) to trigger A/W selection
    ${If} $1 <> 0
        System::Call '*$0(is,i,p,p,p,p,p,p,p,p,p,p,p,p,p.r1)' ;stack value not really used, just a fancy pop ;)
        System::Call 'kernel32::WaitForSingleObject(pr1,i-1)'
        System::Call 'kernel32::GetExitCodeProcess(pr1,*i.s)'
        System::Call 'kernel32::CloseHandle(pr1)'
    ${EndIf}
    System::Free $0
    !if "${exitoutvar}" == ""
        pop $0
    !endif
    System::Store L
    !if "${exitoutvar}" != ""
        pop ${exitoutvar}
    !endif
!macroend

; --------------------------------------------------------------------------------

Var /GLOBAL OldInstallDir
Var /GLOBAL RenameUpgrade
Var /GLOBAL HookNoShortcut
Var /GLOBAL HookNoDesktopShortcut

!macro NSIS_HOOK_PREINSTALL
    ; The updater is normally launched silently by the Tauri updater plugin.
    ; A current-user install can still target a protected directory, so elevate
    ; only the in-place update process instead of requiring UAC for first-time
    ; current-user installs.
    ${If} $UpdateMode == 1
      ${GetOptions} $CMDLINE "/ELEVATED" $0
      ${If} ${Errors}
        UserInfo::GetAccountType
        Pop $0
        ${If} $0 != "Admin"
          ExecShell "runas" "$EXEPATH" '$CMDLINE /ELEVATED'
          Abort
        ${EndIf}
      ${EndIf}
    ${EndIf}

    ; The updater already stops the running process and passes /UPDATE. Do not
    ; run the legacy uninstall/migration path during an in-place update, since
    ; it can resolve a different installation from the registry and relaunch
    ; the old executable after the new files are installed.
    ${If} $UpdateMode <> 1
      SetShellVarContext all
      ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
        UserInfo::GetAccountType
        Pop $0
        ${If} $0 != "Admin"
            MessageBox MB_ICONINFORMATION|MB_OK "An old installation of the Axolotl Launcher was detected that requires administrator permission to update from. You will be prompted with an admin prompt shortly."
        ${EndIf}

        ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
        ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"

        ReadRegStr $OldInstallDir SHCTX "${UNINSTKEY}" "InstallLocation"
        StrCpy $OldInstallDir $OldInstallDir "" 1
        StrCpy $OldInstallDir $OldInstallDir -1 ""

        DetailPrint "Executing $R1"
        !insertmacro ShellExecWait "runas" '$R1' '/P _?=$4' "" ${SW_SHOW} $3
        ${If} $3 <> 0
            SetErrorLevel $3
            MessageBox MB_ICONEXCLAMATION|MB_OK "Failed to uninstall old global installation"
            Abort
        ${EndIf}
      ${EndIf}
      SetShellVarContext current
    ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
    ; Keep an existing desktop shortcut pointing at this installation.
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$OldInstallDir\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
        !insertmacro SetShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    ${EndIf}

    ; Shortcuts are only created for the rename upgrade. The updater runs the
    ; installer with /UPDATE, which makes the stock template skip shortcuts
    ; entirely, so the launcher's new name would never get one. Ordinary
    ; installs and updates keep the stock behaviour, and a shortcut the player
    ; deleted is not recreated.
    ;
    ; The opt-out flags are read from the command line here rather than from the
    ; template's variables, which the bundled template does not define.
    StrCpy $HookNoShortcut 0
    StrCpy $HookNoDesktopShortcut 0
    ${GetOptions} $CMDLINE "/NS" $0
    ${IfNot} ${Errors}
        StrCpy $HookNoShortcut 1
    ${EndIf}
    ${GetOptions} $CMDLINE "/NO_DESKTOP_SHORTCUT" $0
    ${IfNot} ${Errors}
        StrCpy $HookNoDesktopShortcut 1
    ${EndIf}

    StrCpy $RenameUpgrade 0
    ReadRegStr $1 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Axolotl Launcher" "UninstallString"
    ${If} $1 != ""
        StrCpy $RenameUpgrade 1
    ${Else}
        ReadRegStr $1 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Axolotl Launcher" "UninstallString"
        ${If} $1 != ""
            StrCpy $RenameUpgrade 1
        ${EndIf}
    ${EndIf}

    ; The old launcher may already be gone because the data move finished first.
    ${If} $RenameUpgrade = 0
    ${AndIf} ${FileExists} "$APPDATA\club.sitmc.launcher\.legacy-migration-complete"
        StrCpy $RenameUpgrade 1
    ${EndIf}

    ${If} $RenameUpgrade = 1
        ${If} $HookNoShortcut = 0
            ${IfNot} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
                CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
                !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
            ${EndIf}
        ${EndIf}

        ${If} $HookNoDesktopShortcut = 0
            ${IfNot} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
                CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
                !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
            ${EndIf}
        ${EndIf}
    ${EndIf}
!macroend


!macro NSIS_HOOK_PREUNINSTALL
    ${If} $DeleteAppDataCheckboxState = 1
    ${AndIf} $UpdateMode <> 1
        ${If} ${FileExists} "$APPDATA\${BUNDLEID}"
            Push "$APPDATA\${BUNDLEID}"
            Call un.RemoveReparsePoints
        ${EndIf}
        ${If} ${FileExists} "$LOCALAPPDATA\${BUNDLEID}"
            Push "$LOCALAPPDATA\${BUNDLEID}"
            Call un.RemoveReparsePoints
        ${EndIf}
    ${EndIf}
!macroend

Function un.RemoveReparsePoints
    Exch $0 ; root directory to scan
    Push $1 ; FindFirst handle
    Push $2 ; current entry name
    Push $3 ; current entry path
    Push $4 ; entry attributes
    Push $5 ; spare

    FindFirst $1 $2 "$0\*"
    ${If} ${Errors}
        Goto done
    ${EndIf}

    loop:
        StrCmp $2 "." next
        StrCmp $2 ".." next

        StrCpy $3 "$0\$2"
        System::Call 'kernel32::GetFileAttributes(t r3) i.r4'

        IntOp $4 $4 & 0x400 ; FILE_ATTRIBUTE_REPARSE_POINT
        IntCmp $4 0 notReparse isReparse isReparse

        isReparse:
            System::Call 'kernel32::GetFileAttributes(t r3) i.r4'
            IntOp $4 $4 & 0x10 ; FILE_ATTRIBUTE_DIRECTORY
            IntCmp $4 0 removeFileLink removeDirLink removeDirLink

            removeDirLink:
                RmDir "$3" ; removes the junction / directory symlink itself
                Goto next
            removeFileLink:
                Delete "$3" ; removes the file symlink itself
                Goto next

        notReparse:
            System::Call 'kernel32::GetFileAttributes(t r3) i.r4'
            IntOp $4 $4 & 0x10
            IntCmp $4 0 next recurse recurse

            recurse:
                Push $3
                Call un.RemoveReparsePoints

        next:
            FindNext $1 $2
            IfErrors done
            Goto loop

    done:
        FindClose $1
        Pop $5
        Pop $4
        Pop $3
        Pop $2
        Pop $1
        Pop $0
FunctionEnd
