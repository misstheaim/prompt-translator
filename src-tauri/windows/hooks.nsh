; Prompt Translator installer hooks (bundle.windows.nsis.installerHooks, plan T6 / D24).
;
; The autostart entry that tauri-plugin-autostart writes (see src/system/autostart.rs): the value
; `Prompt Translator` (AUTOSTART_NAME there; keep the two in sync) under
;   HKCU\Software\Microsoft\Windows\CurrentVersion\Run
; and, when that key exists, the same value under
;   HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run
; which the plugin never removes. A real uninstall always deletes both, whatever the
; "Delete application data" checkbox says; an update ($UpdateMode) keeps them.
;
; POSTUNINSTALL, not PREUNINSTALL: the uninstall can still stop after PREUNINSTALL (the app is
; running and the user cancels), and the entry must stay while the app is installed.
; The per-user app runs unelevated, so the plugin never writes HKLM; nothing to do there.

!define PT_AUTOSTART_VALUE "Prompt Translator"

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PT_AUTOSTART_VALUE}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PT_AUTOSTART_VALUE}"
  ${EndIf}
!macroend
