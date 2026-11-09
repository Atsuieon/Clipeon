; Dodatki do instalatora Clippera (haczyki szablonu NSIS z Tauri).

!macro NSIS_HOOK_POSTINSTALL
  ; Uruchamianie razem z Windows — pytamy tylko przy zwykłej instalacji (nie przy cichej aktualizacji).
  ; Wpis jest taki sam jak ten, który ustawia przełącznik w Ustawieniach Clippera.
  IfSilent clipper_autostart_koniec
  MessageBox MB_YESNO|MB_ICONQUESTION "Uruchamiać Clippera razem z Windows?$\r$\nClipper będzie wtedy od razu gotowy do zapisywania klipów.$\r$\n$\r$\nStart Clipper together with Windows?" IDNO clipper_autostart_koniec
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Clipper" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  clipper_autostart_koniec:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Przy pełnym odinstalowaniu usuwamy autostart. Przy aktualizacji (/UPDATE) zostawiamy go.
  ; Klipów (folder Wideo\Clipper) nigdy nie ruszamy.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Clipper"
    ; „Usuń dane aplikacji” zaznaczone: także ustawienia i dziennik
    ${If} $DeleteAppDataCheckboxState = 1
      RMDir /r "$APPDATA\Clipper"
    ${EndIf}
  ${EndIf}
!macroend
