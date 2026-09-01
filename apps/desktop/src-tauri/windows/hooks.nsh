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
; inside a Section. The uninstall macro runs inside NSIS's real `Section
; Uninstall`, which forbids `Call`ing any Function not prefixed `un.` — this
; file never declares an NSIS Function at all (see the System::Call note
; below), so that restriction never applies to it.
;
; This hardcodes HKCU (the per-user environment). It is only correct when
; the installer itself is per-user; under perMachine/both an elevated
; installer would run as a different (admin) principal and this would
; silently edit *that* account's PATH instead of the real user's.
!if "${INSTALLMODE}" != "currentUser"
  !error "windows/hooks.nsh hardcodes HKCU for the per-user PATH and is only safe for bundle.windows.nsis.installMode == currentUser; wire a per-machine (HKLM) path or drop this hook before changing installMode."
!endif
;
; Tauri's bundled NSIS toolchain does not ship the third-party EnVar plugin,
; and the one bundled utility plugin (nsis_tauri_utils) has no PATH-related
; exports (verified against the shipped x86-unicode plugin set: FindProcess,
; KillProcess, RunAsUser, SemverCompare, StrReplace — nothing else).
;
; The per-user PATH on a real machine routinely exceeds NSIS's ~1024-
; character string-variable cap (`${NSIS_MAX_STRLEN}`, itself a compiler
; builtin — not something this file can raise, since by the time it is
; `!include`d partway through Tauri's generated installer.nsi, the exehead's
; variable layout is already fixed). Reading such a value with `ReadRegStr`
; does not silently truncate it — it sets the NSIS error flag and yields an
; empty string — so that failure mode was never the risk. The real problem
; is narrower but unavoidable: our own working copy of the value (padded,
; searched, and rebuilt with the new entry) would itself be truncated the
; moment it has to live in a $var, corrupting whatever didn't fit the moment
; it gets written back. So neither the read nor the write ever puts the
; PATH value in an NSIS variable: both go through `System::Call` against
; raw, `GlobalAlloc`-backed memory (`Advapi32::RegQueryValueExW` /
; `RegSetValueExW`, with `Shlwapi::StrStrW` for the substring search), which
; has no such cap. Only short, fixed-size things ($INSTDIR, the literal
; separator) ever pass through a plain NSIS string.
; ---------------------------------------------------------------------------

!ifndef AJH_PATH_HOOKS_INCLUDED
!define AJH_PATH_HOOKS_INCLUDED

!include "LogicLib.nsh"
!include "WinMessages.nsh"
!include "WinCore.nsh"   ; HKEY_CURRENT_USER / HKCU

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

; Registry access rights and error codes (winnt.h / winerror.h) — not
; exported by any bundled NSIS header (only the HKEY_* root constants are).
!define AJH_KEY_QUERY_VALUE 0x0001
!define AJH_KEY_SET_VALUE 0x0002
!define AJH_ERROR_FILE_NOT_FOUND 2

; GMEM_FIXED (0x0000) | GMEM_ZEROINIT (0x0040): a plain, zero-initialized
; block whose handle IS its usable pointer (no GlobalLock needed). The
; zero-init matters — every buffer below is sized exactly to its content
; plus a NUL, and relies on that final NUL already being zero rather than
; writing it explicitly.
!define AJH_GPTR 0x0040

; ---------------------------------------------------------------------------
; Shared: query the current value's type and byte size (including its
; terminating NUL) without reading its content. Populates:
;   $1 = Win32 result code (0 = value exists and was queried successfully)
;   $2 = registry value type   (only meaningful when $1 == 0)
;   $3 = value size in bytes   (only meaningful when $1 == 0)
; $1 is the ONLY thing callers may branch on to decide whether the value
; exists — and only a literal ${AJH_ERROR_FILE_NOT_FOUND} counts as a
; *confirmed* absence. Anything else non-zero (a real permissions error, an
; unexpected Win32 code, or — if a future edit typos an export name —
; System::Call's own "error" string sentinel, which is neither 0 nor a
; small integer) must be treated as "unknown", never as "absent": the
; install-side hook creates a fresh PATH value when absence is confirmed,
; and confusing "unknown" with "absent" there means overwriting the user's
; entire PATH with just $INSTDIR. Only ever opens the key for
; KEY_QUERY_VALUE, never for write, so it cannot itself corrupt anything.
; ---------------------------------------------------------------------------
!macro AJH_QueryPathTypeAndSize
  System::Call 'Advapi32::RegOpenKeyExW(i ${HKEY_CURRENT_USER}, w "${AJH_PATH_REGKEY}", i 0, i ${AJH_KEY_QUERY_VALUE}, *i .r0) i .r1'
  ${If} $1 == 0
    System::Call 'Advapi32::RegQueryValueExW(i r0, w "${AJH_PATH_REGVALUE}", i 0, *i .r2, i 0, *i .r3) i .r1'
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
  Push $8
  Push $9

  StrCpy $1 "" ; unknown until proven otherwise — see AJH_QueryPathTypeAndSize
  !insertmacro AJH_QueryPathTypeAndSize

  ${If} $1 == ${AJH_ERROR_FILE_NOT_FOUND}
    ; Positively confirmed absent (missing key or missing value): safe to
    ; create. $INSTDIR is always short, so this is the one place a plain
    ; NSIS string write is fine. REG_EXPAND_SZ matches what Windows itself
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

  ${If} $1 != 0
    ; Not a confirmed absence — do nothing rather than guess (see the macro
    ; doc comment above for why this must never fall through to "create").
    DetailPrint "ajh: could not determine whether a per-user PATH value exists; leaving it untouched"
    Goto ajh_addpath_done
  ${EndIf}

  ${If} $2 != ${AJH_REG_SZ}
  ${AndIf} $2 != ${AJH_REG_EXPAND_SZ}
    DetailPrint "ajh: per-user PATH has an unexpected registry type; leaving it untouched"
    Goto ajh_addpath_done
  ${EndIf}

  ; From here: the value exists, type is SZ/EXPAND_SZ ($2), size is $3 bytes
  ; (including its NUL). Build padBuf ($4) = ';' + <value> + ';' + NUL in
  ; raw memory, so a single substring search proves *whole-segment*
  ; membership — a sibling directory sharing our prefix (e.g.
  ; "...\AI Job HunterX") cannot match ";$INSTDIR;". $3 bytes already
  ; include the value's own NUL, so padBuf needs exactly $3+4 bytes (one
  ; ';' added on each side, reusing the space the original NUL occupied).
  IntOp $0 $3 + 4
  System::Call 'Kernel32::GlobalAlloc(i ${AJH_GPTR}, i r0) i .r4'
  System::Call 'Kernel32::lstrcpyW(i r4, w ";") i .r1'

  IntOp $0 $4 + 2
  System::Call 'Advapi32::RegOpenKeyExW(i ${HKEY_CURRENT_USER}, w "${AJH_PATH_REGKEY}", i 0, i ${AJH_KEY_QUERY_VALUE}, *i .r5) i .r1'
  System::Call 'Advapi32::RegQueryValueExW(i r5, w "${AJH_PATH_REGVALUE}", i 0, i 0, i r0, *i r3) i .r1'
  System::Call 'Advapi32::RegCloseKey(i r5)'
  ${If} $1 != 0
    DetailPrint "ajh: could not re-read the per-user PATH; leaving it untouched"
    System::Call 'Kernel32::GlobalFree(i r4)'
    Goto ajh_addpath_done
  ${EndIf}

  ; Overwrite the value's own NUL (now sitting at padBuf+$3) with the
  ; trailing pad ';', then a fresh NUL right after — both fit in the extra
  ; 4 bytes allocated above.
  IntOp $0 $4 + $3
  System::Call 'Kernel32::lstrcpyW(i r0, w ";") i .r5'

  ; Whole-segment membership check (idempotency).
  System::Call 'Shlwapi::StrStrW(i r4, w ";$INSTDIR;") i .r5'
  ${If} $5 != 0
    DetailPrint "ajh: agent CLI directory is already on the per-user PATH"
    System::Call 'Kernel32::GlobalFree(i r4)'
    Goto ajh_addpath_done
  ${EndIf}

  ; Not present: build newBuf ($7) = <original value> + ';' + $INSTDIR + NUL.
  StrLen $6 ";$INSTDIR"
  IntOp $6 $6 + 1
  IntOp $6 $6 * 2
  IntOp $0 $3 - 2
  IntOp $6 $6 + $0                ; $6 = newBytes

  System::Call 'Kernel32::GlobalAlloc(i ${AJH_GPTR}, i r6) i .r7'
  IntOp $0 $4 + 2                 ; original value starts right after the leading pad
  IntOp $8 $3 - 2                 ; its length in bytes, excluding its own NUL
  System::Call 'Kernel32::RtlMoveMemory(i r7, i r0, i r8)'
  IntOp $0 $7 + $8
  System::Call 'Kernel32::lstrcpyW(i r0, w ";$INSTDIR") i .r5'

  ClearErrors
  System::Call 'Advapi32::RegOpenKeyExW(i ${HKEY_CURRENT_USER}, w "${AJH_PATH_REGKEY}", i 0, i ${AJH_KEY_SET_VALUE}, *i .r0) i .r1'
  ${If} $1 == 0
    System::Call 'Advapi32::RegSetValueExW(i r0, w "${AJH_PATH_REGVALUE}", i 0, i r2, i r7, i r6) i .r1'
    System::Call 'Advapi32::RegCloseKey(i r0)'
  ${EndIf}
  ${If} $1 == 0
    DetailPrint "ajh: added the agent CLI directory to the per-user PATH"
    !insertmacro AJH_BroadcastEnvironmentChange
  ${Else}
    DetailPrint "ajh: failed to update the per-user PATH; leaving it untouched"
  ${EndIf}
  System::Call 'Kernel32::GlobalFree(i r7)'
  System::Call 'Kernel32::GlobalFree(i r4)'

  ajh_addpath_done:
  Pop $9
  Pop $8
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
  Push $8
  Push $9

  StrCpy $1 ""
  !insertmacro AJH_QueryPathTypeAndSize

  ${If} $1 != 0
    ; Either confirmed absent (${AJH_ERROR_FILE_NOT_FOUND}) or unknown (any
    ; other code) — unlike install, both cases are safe to treat the same
    ; way here: skipping a cleanup never destroys anything.
    DetailPrint "ajh: no per-user PATH value to clean up"
    Goto ajh_rmpath_done
  ${EndIf}

  ${If} $2 != ${AJH_REG_SZ}
  ${AndIf} $2 != ${AJH_REG_EXPAND_SZ}
    DetailPrint "ajh: per-user PATH has an unexpected registry type; leaving it untouched"
    Goto ajh_rmpath_done
  ${EndIf}

  ; padBuf ($4), built the same way as install — see NSIS_HOOK_POSTINSTALL
  ; for the full rationale on why this goes through raw memory.
  IntOp $0 $3 + 4
  System::Call 'Kernel32::GlobalAlloc(i ${AJH_GPTR}, i r0) i .r4'
  System::Call 'Kernel32::lstrcpyW(i r4, w ";") i .r1'
  IntOp $0 $4 + 2
  System::Call 'Advapi32::RegOpenKeyExW(i ${HKEY_CURRENT_USER}, w "${AJH_PATH_REGKEY}", i 0, i ${AJH_KEY_QUERY_VALUE}, *i .r5) i .r1'
  System::Call 'Advapi32::RegQueryValueExW(i r5, w "${AJH_PATH_REGVALUE}", i 0, i 0, i r0, *i r3) i .r1'
  System::Call 'Advapi32::RegCloseKey(i r5)'
  ${If} $1 != 0
    DetailPrint "ajh: could not re-read the per-user PATH; leaving it untouched"
    System::Call 'Kernel32::GlobalFree(i r4)'
    Goto ajh_rmpath_done
  ${EndIf}
  IntOp $0 $4 + $3
  System::Call 'Kernel32::lstrcpyW(i r0, w ";") i .r5'

  System::Call 'Shlwapi::StrStrW(i r4, w ";$INSTDIR;") i .r5'
  ${If} $5 == 0
    DetailPrint "ajh: agent CLI directory is not on the per-user PATH; nothing to remove"
    System::Call 'Kernel32::GlobalFree(i r4)'
    Goto ajh_rmpath_done
  ${EndIf}

  ; Splice out exactly our segment plus the one delimiter it owns on each
  ; side. $6 (prefixLen) and $7 (suffixLen) legitimately come out as 0 when
  ; our entry is first/last/only — the two ${If}/clamp pairs below exist
  ; *because* the naive formula goes negative in exactly those cases (a
  ; negative length handed to RtlMoveMemory is a huge unsigned count and
  ; crashes the uninstaller). $1 (suffixPtr) must stay live and unclobbered
  ; all the way to the final RtlMoveMemory call below — an earlier version
  ; of this file reused $1 as the throwaway output register for the
  ; separator write two steps later, which silently corrupted every removal
  ; except when the removed entry was last; $5 is the correct throwaway
  ; there instead, since matchPtr is never read again after this point.
  StrLen $0 ";$INSTDIR;"
  IntOp $0 $0 * 2                  ; $0 = needleBytes

  IntOp $6 $5 - $4                 ; raw offset of the match from padBuf's start
  ${If} $6 > 0
    IntOp $6 $6 - 2                ; drop the leading pad ';' -> prefixLen
  ${EndIf}                          ; ($6 stays 0 when our entry is first)

  IntOp $1 $5 + $0                 ; $1 = suffixPtr
  IntOp $9 $4 + $3                 ; original NUL position (= end of real content)
  IntOp $7 $9 - $1                 ; raw suffixLen
  ${If} $7 < 0
    StrCpy $7 0                    ; our entry was last: nothing legitimate follows
  ${EndIf}

  IntOp $9 $6 + $7
  ${If} $6 > 0
  ${AndIf} $7 > 0
    IntOp $9 $9 + 2                 ; a separator is only needed between two real neighbours
  ${EndIf}
  IntOp $9 $9 + 2                   ; NUL terminator

  System::Call 'Kernel32::GlobalAlloc(i ${AJH_GPTR}, i r9) i .r8'

  IntOp $0 $4 + 2
  System::Call 'Kernel32::RtlMoveMemory(i r8, i r0, i r6)'
  IntOp $0 $8 + $6

  ${If} $6 > 0
  ${AndIf} $7 > 0
    System::Call 'Kernel32::lstrcpyW(i r0, w ";") i .r5'
    IntOp $0 $0 + 2
  ${EndIf}

  System::Call 'Kernel32::RtlMoveMemory(i r0, i r1, i r7)'

  ClearErrors
  System::Call 'Advapi32::RegOpenKeyExW(i ${HKEY_CURRENT_USER}, w "${AJH_PATH_REGKEY}", i 0, i ${AJH_KEY_SET_VALUE}, *i .r0) i .r1'
  ${If} $1 == 0
    System::Call 'Advapi32::RegSetValueExW(i r0, w "${AJH_PATH_REGVALUE}", i 0, i r2, i r8, i r9) i .r1'
    System::Call 'Advapi32::RegCloseKey(i r0)'
  ${EndIf}
  ${If} $1 == 0
    DetailPrint "ajh: removed the agent CLI directory from the per-user PATH"
    !insertmacro AJH_BroadcastEnvironmentChange
  ${Else}
    DetailPrint "ajh: failed to update the per-user PATH; leaving it untouched"
  ${EndIf}

  System::Call 'Kernel32::GlobalFree(i r8)'
  System::Call 'Kernel32::GlobalFree(i r4)'

  ajh_rmpath_done:
  Pop $9
  Pop $8
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
