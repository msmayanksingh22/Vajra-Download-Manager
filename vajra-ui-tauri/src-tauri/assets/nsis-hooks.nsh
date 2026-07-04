!include "MUI2.nsh"

; ── Vajra PATH + vdm alias + Native Messaging hooks ──────────────────────────
; These macros are called by the Tauri-generated NSIS installer automatically.

!macro customInstall
  ; Add install dir to HKLM Environment PATH so 'vajra' works for all users
  ReadRegStr $0 HKLM "System\CurrentControlSet\Control\Session Manager\Environment" "Path"
  Push $0
  Push "$INSTDIR"
  Call StrContains
  Pop $1
  StrCmp $1 "" 0 path_already_set
    StrCpy $0 "$0;$INSTDIR"
    WriteRegExpandStr HKLM "System\CurrentControlSet\Control\Session Manager\Environment" "Path" $0
  path_already_set:

  ; Create vdm.bat alias
  FileOpen $1 "$INSTDIR\vdm.bat" w
  FileWrite $1 "@echo off$\r$\n"
  FileWrite $1 '"$INSTDIR\vajra-cli.exe" %*$\r$\n'
  FileClose $1

  ; Create vajra.bat alias
  FileOpen $1 "$INSTDIR\vajra.bat" w
  FileWrite $1 "@echo off$\r$\n"
  FileWrite $1 '"$INSTDIR\vajra-cli.exe" %*$\r$\n'
  FileClose $1

  ; Broadcast WM_SETTINGCHANGE so new PATH is live
  System::Call 'user32::SendMessageTimeout(i 0xffff, i 0x001A, i 0, t "Environment", i 2, i 5000, *i .r0)'

  ; Create Native Messaging Host script
  FileOpen $1 "$INSTDIR\native-host.bat" w
  FileWrite $1 "@echo off$\r$\n"
  FileWrite $1 'start "" "$INSTDIR\vajra-ui-tauri.exe" --minimized$\r$\n'
  FileWrite $1 "exit 0$\r$\n"
  FileClose $1

  ; Replace single backslashes with double backslashes for JSON
  Push $INSTDIR
  Call EscapeBackslashes
  Pop $2

  ; Create Native Messaging JSON manifest
  FileOpen $1 "$INSTDIR\com.vajra.manager.json" w
  FileWrite $1 '{$\r$\n'
  FileWrite $1 '  "name": "com.vajra.manager",$\r$\n'
  FileWrite $1 '  "description": "Vajra Native Messaging Host",$\r$\n'
  FileWrite $1 '  "path": "$2\\\\native-host.bat",$\r$\n'
  FileWrite $1 '  "type": "stdio",$\r$\n'
  FileWrite $1 '  "allowed_origins": [$\r$\n'
  FileWrite $1 '    "chrome-extension://mfdepghakanbpamaakojoaogglepehfh/"$\r$\n'
  FileWrite $1 '  ]$\r$\n'
  FileWrite $1 '}$\r$\n'
  FileClose $1

  ; Register in Chrome and Edge registries
  WriteRegStr HKLM "Software\Google\Chrome\NativeMessagingHosts\com.vajra.manager" "" "$INSTDIR\com.vajra.manager.json"
  WriteRegStr HKLM "Software\Microsoft\Edge\NativeMessagingHosts\com.vajra.manager" "" "$INSTDIR\com.vajra.manager.json"
!macroend

!macro customUnInstall
  ; Remove the aliases
  Delete "$INSTDIR\vdm.bat"
  Delete "$INSTDIR\vajra.bat"
  Delete "$INSTDIR\native-host.bat"
  Delete "$INSTDIR\com.vajra.manager.json"

  ; Remove registries
  DeleteRegKey HKLM "Software\Google\Chrome\NativeMessagingHosts\com.vajra.manager"
  DeleteRegKey HKLM "Software\Microsoft\Edge\NativeMessagingHosts\com.vajra.manager"
!macroend

; ── Helper: StrContains ───────────────────────────────────────────────────────
Function StrContains
  Exch $R0 ; needle
  Exch
  Exch $R1 ; haystack
  Push $R2
  Push $R3
  StrLen $R3 $R0
  IntOp $R3 $R3 - 1
  loop:
    StrCpy $R2 $R1 $R3
    StrCmp $R2 $R0 found
    StrCmp $R1 "" notfound
    StrCpy $R1 $R1 "" 1
    Goto loop
  found:
    StrCpy $R0 $R1
    Goto done
  notfound:
    StrCpy $R0 ""
  done:
    Pop $R3
    Pop $R2
    Pop $R1
    Exch $R0
FunctionEnd

; ── Helper: EscapeBackslashes ─────────────────────────────────────────────────
Function EscapeBackslashes
  Exch $R0
  Push $R1
  Push $R2
  StrCpy $R1 ""
  loop:
    StrCmp $R0 "" done
    StrCpy $R2 $R0 1
    StrCpy $R0 $R0 "" 1
    StrCmp $R2 "\" 0 +2
    StrCpy $R2 "\\\\"
    StrCpy $R1 "$R1$R2"
    Goto loop
  done:
    StrCpy $R0 $R1
    Pop $R2
    Pop $R1
    Exch $R0
FunctionEnd
