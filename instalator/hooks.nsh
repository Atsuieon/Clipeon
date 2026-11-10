; Dodatki do instalatora Clipeona (haczyki szablonu NSIS z Tauri).

!macro NSIS_HOOK_PREINSTALL
  ; Przejście z Clippera 1.0 (dawna nazwa programu): stary program jest zamykany i odinstalowywany
  ; po cichu, żeby nie zostały dwie kopie. Klipy i ustawienia zostają (Clipeon przejmie je przy starcie).
  ReadRegStr $0 HKCU "Software\Atsuieon\Clipper" ""
  ${If} $0 == ""
    StrCpy $0 "$LOCALAPPDATA\Clipper"
  ${EndIf}
  ${If} ${FileExists} "$0\uninstall.exe"
    DetailPrint "Usuwanie Clippera 1.0 (poprzednia nazwa programu)..."
    ; _?= — deinstalator działa w miejscu i instalator czeka na jego koniec
    ExecWait '"$0\uninstall.exe" /S _?=$0'
    Delete "$0\uninstall.exe"
    RMDir "$0"
  ${EndIf}
  DeleteRegKey HKCU "Software\Atsuieon\Clipper"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Clipper"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Clipper"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Uruchamianie razem z Windows — pytamy tylko przy zwykłej instalacji (nie przy cichej ani przy
  ; aktualizacji z poziomu Clipeona, /UPDATE). Wpis jest taki sam jak ten z Ustawień Clipeona.
  IfSilent clipeon_autostart_koniec
  ${If} $UpdateMode = 1
    Goto clipeon_autostart_koniec
  ${EndIf}
  MessageBox MB_YESNO|MB_ICONQUESTION "Uruchamiać Clipeona razem z Windows?$\r$\nClipeon będzie wtedy od razu gotowy do zapisywania klipów.$\r$\n$\r$\nStart Clipeon together with Windows?" IDNO clipeon_autostart_koniec
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Clipeon" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  clipeon_autostart_koniec:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Przy pełnym odinstalowaniu usuwamy autostart. Przy aktualizacji (/UPDATE) zostawiamy go.
  ; Klipów nigdy nie ruszamy.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Clipeon"
    ; „Usuń dane aplikacji” zaznaczone: także ustawienia i dziennik
    ${If} $DeleteAppDataCheckboxState = 1
      RMDir /r "$APPDATA\Clipeon"
    ${EndIf}
  ${EndIf}
!macroend
