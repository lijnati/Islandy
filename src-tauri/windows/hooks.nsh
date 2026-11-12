; Islandy NSIS hooks (bundle.windows.nsis.installerHooks in tauri.conf.json).
;
; On uninstall (not when an update replaces the app, i.e. $UpdateMode <> 1):
;   1. `<main binary>.exe uninstall-cleanup` removes Islandy's hooks from the
;      user-level Claude Code settings (backup first; other hooks untouched)
;      and the sign-in entry. It prints nothing and always exits 0.
;   2. As a fallback, the sign-in entry is deleted here as well.
; Nothing else in the registry belongs to Islandy (the uninstaller removes
; its own uninstall key and shortcuts).

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 +2
      nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" uninstall-cleanup'
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Islandy"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Islandy"
  ${EndIf}
!macroend
