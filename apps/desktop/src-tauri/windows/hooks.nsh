; ---------------------------------------------------------------------------
; NSIS installer hooks: put the agent CLI (`ajh-tauri agent <verb>`, see
; docs/knowledge/agent-cli.md and ADR-037/038) on the per-user PATH so a
; human (or a shell an agent drives) can type it directly. The app already
; writes a machine-discoverable pointer file on every launch
; (extension_bridge/register.rs, write_agent_pointer) — this hook exists
; purely for interactive shell use, which the pointer file cannot provide.
;
; Wired via bundle.windows.nsis.installerHooks in tauri.conf.json. Hooked
; into the generated installer.nsi as:
;   !ifmacrodef NSIS_HOOK_POSTINSTALL   -> !insertmacro NSIS_HOOK_POSTINSTALL
;   !ifmacrodef NSIS_HOOK_POSTUNINSTALL -> !insertmacro NSIS_HOOK_POSTUNINSTALL
; both pasted inline into the installer's/uninstaller's Section body, so
; $INSTDIR is already valid and these must stay plain instructions — no
; `Function`/`FunctionEnd` blocks here, since a Function cannot be nested
; inside a Section.
;
; Tauri's bundled NSIS toolchain does not ship the third-party EnVar plugin,
; and the one bundled utility plugin (nsis_tauri_utils) has no PATH-related
; exports (verified against the shipped x86-unicode plugin set: FindProcess,
; KillProcess, RunAsUser, SemverCompare, StrReplace — nothing else). So this
; is implemented with core NSIS instructions plus the already-available
; System plugin for the one thing core NSIS cannot do: reading a registry
; value's type (REG_SZ vs REG_EXPAND_SZ) without guessing.
;
; This mutates a registry value whose corruption breaks the user's shell, so
; every step below is written defensively: a failure anywhere just leaves
; PATH untouched and logs via DetailPrint. Nothing here may abort the
; install/uninstall.
; ---------------------------------------------------------------------------

!ifndef AJH_PATH_HOOKS_INCLUDED
!define AJH_PATH_HOOKS_INCLUDED

!include "LogicLib.nsh"
!include "WinMessages.nsh"
!include "WinCore.nsh"   ; HKEY_CURRENT_USER / HKCU
!include "StrFunc.nsh"
${Using:StrFunc} StrLoc  ; no-op if the outer installer.nsi already did this (it does)

; The per-user environment key/value Windows itself uses for PATH. Named so
; a future edit that touches the wrong value is obvious in review. `/ifndef`
; lets a local verification build override these (e.g. via `makensis -D...`)
; to point at a scratch registry key instead of the real per-user PATH.
!define /ifndef AJH_PATH_REGKEY "Environment"
!define /ifndef AJH_PATH_REGVALUE "Path"

; RegQueryValueEx's REG_* type constants we care about (winnt.h / winreg.h).
; Anything else (REG_MULTI_SZ, REG_BINARY, ...) is not a shape we understand
; well enough to safely rewrite, so both hooks below treat it as "leave
; untouched" rather than guess.
!define AJH_REG_SZ 1
!define AJH_REG_EXPAND_SZ 2

; Not exported by any bundled NSIS header (only the HKEY_* root constants
; are); value from winnt.h.
!define AJH_KEY_QUERY_VALUE 0x0001

; NSIS variables (and values crossing the plugin-call stack) are capped at
; this many characters in the Unicode NSIS build Tauri bundles — see
; NSIS_MAX_STRLEN in the NSIS source (Source/exehead/config.h), 1024 by
; default and not queryable from script. `ReadRegStr` on a value longer than
; this SILENTLY TRUNCATES what lands in the variable, so a value read this
; close to the cap cannot be trusted to be the real, complete PATH — writing
; it back (with or without our own edit) risks permanently dropping whatever
; didn't fit. Both hooks below refuse to touch PATH once its length is
; within one character of this cap, rather than risk that.
!define AJH_NSIS_MAX_STRLEN 1024

; Extra headroom below the cap that any *new* value must also respect: our
; separator plus a little slack, so we never write a result flush against
; the truncation boundary above.
!define AJH_PATH_SAFETY_MARGIN 8

; Pre-computed at compile time (LogicLib's ${If} takes a single comparison
; value, not an inline expression, so the arithmetic has to happen here).
!define /math AJH_PATH_TRUNCATION_GUARD ${AJH_NSIS_MAX_STRLEN} - 1
!define /math AJH_PATH_APPEND_BUDGET ${AJH_NSIS_MAX_STRLEN} - ${AJH_PATH_SAFETY_MARGIN}

; ---------------------------------------------------------------------------
; Shared: query the current type of AJH_PATH_REGVALUE (REG_SZ / REG_EXPAND_SZ
; / absent / anything else), leaving the type in $2 and the Win32 result
; code in $1 (0 = success; non-zero means absent-or-unreadable — callers
; branch on this rather than the type when $1 != 0). Only ever opens the key
; for KEY_QUERY_VALUE, never for write, so it cannot itself corrupt anything.
; ---------------------------------------------------------------------------
!macro AJH_QueryPathType
  System::Call 'Advapi32::RegOpenKeyExW(i ${HKEY_CURRENT_USER}, w "${AJH_PATH_REGKEY}", i 0, i ${AJH_KEY_QUERY_VALUE}, *i .r0) i .r1'
  ${If} $1 == 0
    System::Call 'Advapi32::RegQueryValueExW(i r0, w "${AJH_PATH_REGVALUE}", i 0, *i .r2, i 0, i 0) i .r1'
    System::Call 'Advapi32::RegCloseKey(i r0)'
  ${EndIf}
!macroend

; Best-effort environment-change broadcast so already-open shells pick up
; the new PATH without a reboot. `/TIMEOUT` makes NSIS use SendMessageTimeout
; internally so one wedged top-level window cannot hang the (un)installer.
!macro AJH_BroadcastEnvironmentChange
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  Push $7

  !insertmacro AJH_QueryPathType

  ${If} $1 != 0
    ; No existing Path value (or the Environment key itself is new): create
    ; a fresh single-entry value. REG_EXPAND_SZ matches what Windows itself
    ; writes for a brand-new per-user PATH.
    ClearErrors
    WriteRegExpandStr HKCU "${AJH_PATH_REGKEY}" "${AJH_PATH_REGVALUE}" "$INSTDIR"
    ${IfNot} ${Errors}
      DetailPrint "ajh: added the agent CLI directory to the per-user PATH"
      !insertmacro AJH_BroadcastEnvironmentChange
    ${Else}
      DetailPrint "ajh: could not create a per-user PATH value; leaving PATH untouched"
    ${EndIf}
    Goto ajh_addpath_done
  ${EndIf}

  ${If} $2 != ${AJH_REG_SZ}
  ${AndIf} $2 != ${AJH_REG_EXPAND_SZ}
    DetailPrint "ajh: per-user PATH has an unexpected registry type; leaving it untouched"
    Goto ajh_addpath_done
  ${EndIf}

  ClearErrors
  ReadRegStr $3 HKCU "${AJH_PATH_REGKEY}" "${AJH_PATH_REGVALUE}"
  ${If} ${Errors}
    DetailPrint "ajh: could not read the existing per-user PATH; leaving it untouched"
    Goto ajh_addpath_done
  ${EndIf}

  StrLen $4 $3
  ${If} $4 >= ${AJH_PATH_TRUNCATION_GUARD}
    DetailPrint "ajh: per-user PATH is at NSIS's string-length limit and may already be truncated; leaving it untouched"
    Goto ajh_addpath_done
  ${EndIf}

  ; Idempotency: match $INSTDIR as a whole ';'-delimited segment, not a
  ; substring. Padding both sides with ';' turns this into a plain substring
  ; search that cannot false-match a sibling directory sharing our prefix
  ; (e.g. "...\AI Job HunterX" must not count as already present).
  StrCpy $5 ";$3;"
  ${StrLoc} $6 $5 ";$INSTDIR;" ">"
  ${If} $6 != ""
    DetailPrint "ajh: agent CLI directory is already on the per-user PATH"
    Goto ajh_addpath_done
  ${EndIf}

  ; The appended result must also stay inside the same budget, for the same
  ; truncation reason as above.
  StrLen $6 "$INSTDIR"
  IntOp $6 $6 + 1 ; the separator we're about to add
  IntOp $6 $6 + $4
  ${If} $6 >= ${AJH_PATH_APPEND_BUDGET}
    DetailPrint "ajh: appending to the per-user PATH would approach NSIS's string-length limit; leaving it untouched"
    Goto ajh_addpath_done
  ${EndIf}

  StrCpy $7 $3 1 -1
  ${If} $7 == ";"
    StrCpy $5 "$3$INSTDIR"
  ${Else}
    StrCpy $5 "$3;$INSTDIR"
  ${EndIf}

  ClearErrors
  ${If} $2 == ${AJH_REG_EXPAND_SZ}
    WriteRegExpandStr HKCU "${AJH_PATH_REGKEY}" "${AJH_PATH_REGVALUE}" $5
  ${Else}
    WriteRegStr HKCU "${AJH_PATH_REGKEY}" "${AJH_PATH_REGVALUE}" $5
  ${EndIf}
  ${IfNot} ${Errors}
    DetailPrint "ajh: added the agent CLI directory to the per-user PATH"
    !insertmacro AJH_BroadcastEnvironmentChange
  ${Else}
    DetailPrint "ajh: failed to update the per-user PATH; leaving it untouched"
  ${EndIf}

  ajh_addpath_done:
  Pop $7
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  Push $7

  !insertmacro AJH_QueryPathType

  ${If} $1 != 0
    DetailPrint "ajh: no per-user PATH value to clean up"
    Goto ajh_rmpath_done
  ${EndIf}

  ${If} $2 != ${AJH_REG_SZ}
  ${AndIf} $2 != ${AJH_REG_EXPAND_SZ}
    DetailPrint "ajh: per-user PATH has an unexpected registry type; leaving it untouched"
    Goto ajh_rmpath_done
  ${EndIf}

  ClearErrors
  ReadRegStr $3 HKCU "${AJH_PATH_REGKEY}" "${AJH_PATH_REGVALUE}"
  ${If} ${Errors}
    DetailPrint "ajh: could not read the existing per-user PATH; leaving it untouched"
    Goto ajh_rmpath_done
  ${EndIf}

  StrLen $4 $3
  ${If} $4 >= ${AJH_PATH_TRUNCATION_GUARD}
    ; Same truncation risk as the install-time guard: a value read this
    ; close to NSIS's cap may already be an incomplete copy, so rewriting
    ; anything derived from it could drop entries that never made it in.
    DetailPrint "ajh: per-user PATH is at NSIS's string-length limit and may already be truncated; leaving it untouched"
    Goto ajh_rmpath_done
  ${EndIf}

  ; Same padded-segment search as install; the match's start offset ($6)
  ; lets us splice the entry out including exactly the one delimiter on
  ; each side it owns, never a neighbour's.
  StrCpy $5 ";$3;"
  ${StrLoc} $6 $5 ";$INSTDIR;" ">"
  ${If} $6 == ""
    DetailPrint "ajh: agent CLI directory is not on the per-user PATH; nothing to remove"
    Goto ajh_rmpath_done
  ${EndIf}

  StrCpy $7 $5 $6 ; everything before the match, still ';'-prefixed (or empty)
  ${If} $7 != ""
    StrCpy $7 $7 "" 1 ; drop the leading ';' that isn't ours to keep
  ${EndIf}

  StrLen $4 ";$INSTDIR;"
  IntOp $4 $6 + $4
  StrCpy $3 $5 "" $4 ; everything after the match, still ';'-suffixed (or empty)
  ${If} $3 != ""
    StrCpy $3 $3 -1 ; drop the trailing ';' that isn't ours to keep
  ${EndIf}

  ${If} $7 == ""
    StrCpy $5 $3
  ${ElseIf} $3 == ""
    StrCpy $5 $7
  ${Else}
    StrCpy $5 "$7;$3"
  ${EndIf}

  ClearErrors
  ${If} $2 == ${AJH_REG_EXPAND_SZ}
    WriteRegExpandStr HKCU "${AJH_PATH_REGKEY}" "${AJH_PATH_REGVALUE}" $5
  ${Else}
    WriteRegStr HKCU "${AJH_PATH_REGKEY}" "${AJH_PATH_REGVALUE}" $5
  ${EndIf}
  ${IfNot} ${Errors}
    DetailPrint "ajh: removed the agent CLI directory from the per-user PATH"
    !insertmacro AJH_BroadcastEnvironmentChange
  ${Else}
    DetailPrint "ajh: failed to update the per-user PATH; leaving it untouched"
  ${EndIf}

  ajh_rmpath_done:
  Pop $7
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!endif ; AJH_PATH_HOOKS_INCLUDED
