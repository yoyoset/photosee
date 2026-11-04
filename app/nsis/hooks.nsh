; NSIS hooks for the PhotoSee installer (Tauri `installerHooks`).
;
; Why this instead of `bundle.fileAssociations`: Tauri's generated script uses
; the stock APP_ASSOCIATE macro, which overwrites the default value of
; HKCU\Software\Classes\.ext with PhotoSee's ProgId (and parks the old one in
; a `_backup` value). That is "make me the default handler", which the brief
; forbids. Here PhotoSee is only *registered*: a ProgId per family, an entry
; in each extension's OpenWithProgids ("Open with" list), and Capabilities +
; RegisteredApplications so it shows up under Settings > Apps > Default apps.
; Windows 10/11 then asks the user, and an existing UserChoice is untouched.
;
; KEEP IN SYNC with core/src/format.rs `from_extension`; scripts/package.ps1
; fails the build when the two lists differ.

!define PS_IMG "PhotoSee.Image"
!define PS_RAW "PhotoSee.Raw"
!define PS_CAPS "Software\PhotoSee\Capabilities"

!macro PS_ASSOC EXT PROGID
  WriteRegStr SHCTX "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}" ""
  WriteRegStr SHCTX "${PS_CAPS}\FileAssociations" ".${EXT}" "${PROGID}"
!macroend

!macro PS_UNASSOC EXT PROGID
  DeleteRegValue SHCTX "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}"
  ; No empty husks left behind for a type nobody else registered.
  DeleteRegKey /ifempty SHCTX "Software\Classes\.${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty SHCTX "Software\Classes\.${EXT}"
!macroend

!macro PS_PROGID PROGID TITLE
  WriteRegStr SHCTX "Software\Classes\${PROGID}" "" "${TITLE}"
  WriteRegStr SHCTX "Software\Classes\${PROGID}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr SHCTX "Software\Classes\${PROGID}\shell\open" "" "Open with ${PRODUCTNAME}"
  WriteRegStr SHCTX "Software\Classes\${PROGID}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro PS_PROGID "${PS_IMG}" "PhotoSee image"
  !insertmacro PS_PROGID "${PS_RAW}" "PhotoSee RAW photo"
  WriteRegStr SHCTX "${PS_CAPS}" "ApplicationName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "${PS_CAPS}" "ApplicationDescription" "A minimal, very fast image viewer"
  WriteRegStr SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}" "${PS_CAPS}"
  !insertmacro PS_ASSOC "jpg" "${PS_IMG}"
  !insertmacro PS_ASSOC "jpeg" "${PS_IMG}"
  !insertmacro PS_ASSOC "jpe" "${PS_IMG}"
  !insertmacro PS_ASSOC "jfif" "${PS_IMG}"
  !insertmacro PS_ASSOC "png" "${PS_IMG}"
  !insertmacro PS_ASSOC "gif" "${PS_IMG}"
  !insertmacro PS_ASSOC "webp" "${PS_IMG}"
  !insertmacro PS_ASSOC "avif" "${PS_IMG}"
  !insertmacro PS_ASSOC "bmp" "${PS_IMG}"
  !insertmacro PS_ASSOC "ico" "${PS_IMG}"
  !insertmacro PS_ASSOC "svg" "${PS_IMG}"
  !insertmacro PS_ASSOC "tif" "${PS_IMG}"
  !insertmacro PS_ASSOC "tiff" "${PS_IMG}"
  !insertmacro PS_ASSOC "jxl" "${PS_IMG}"
  !insertmacro PS_ASSOC "psd" "${PS_IMG}"
  !insertmacro PS_ASSOC "heic" "${PS_IMG}"
  !insertmacro PS_ASSOC "heif" "${PS_IMG}"
  !insertmacro PS_ASSOC "hif" "${PS_IMG}"
  !insertmacro PS_ASSOC "cr2" "${PS_RAW}"
  !insertmacro PS_ASSOC "cr3" "${PS_RAW}"
  !insertmacro PS_ASSOC "nef" "${PS_RAW}"
  !insertmacro PS_ASSOC "nrw" "${PS_RAW}"
  !insertmacro PS_ASSOC "arw" "${PS_RAW}"
  !insertmacro PS_ASSOC "srf" "${PS_RAW}"
  !insertmacro PS_ASSOC "sr2" "${PS_RAW}"
  !insertmacro PS_ASSOC "dng" "${PS_RAW}"
  !insertmacro PS_ASSOC "orf" "${PS_RAW}"
  !insertmacro PS_ASSOC "rw2" "${PS_RAW}"
  !insertmacro PS_ASSOC "raf" "${PS_RAW}"
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro PS_UNASSOC "jpg" "${PS_IMG}"
  !insertmacro PS_UNASSOC "jpeg" "${PS_IMG}"
  !insertmacro PS_UNASSOC "jpe" "${PS_IMG}"
  !insertmacro PS_UNASSOC "jfif" "${PS_IMG}"
  !insertmacro PS_UNASSOC "png" "${PS_IMG}"
  !insertmacro PS_UNASSOC "gif" "${PS_IMG}"
  !insertmacro PS_UNASSOC "webp" "${PS_IMG}"
  !insertmacro PS_UNASSOC "avif" "${PS_IMG}"
  !insertmacro PS_UNASSOC "bmp" "${PS_IMG}"
  !insertmacro PS_UNASSOC "ico" "${PS_IMG}"
  !insertmacro PS_UNASSOC "svg" "${PS_IMG}"
  !insertmacro PS_UNASSOC "tif" "${PS_IMG}"
  !insertmacro PS_UNASSOC "tiff" "${PS_IMG}"
  !insertmacro PS_UNASSOC "jxl" "${PS_IMG}"
  !insertmacro PS_UNASSOC "psd" "${PS_IMG}"
  !insertmacro PS_UNASSOC "heic" "${PS_IMG}"
  !insertmacro PS_UNASSOC "heif" "${PS_IMG}"
  !insertmacro PS_UNASSOC "hif" "${PS_IMG}"
  !insertmacro PS_UNASSOC "cr2" "${PS_RAW}"
  !insertmacro PS_UNASSOC "cr3" "${PS_RAW}"
  !insertmacro PS_UNASSOC "nef" "${PS_RAW}"
  !insertmacro PS_UNASSOC "nrw" "${PS_RAW}"
  !insertmacro PS_UNASSOC "arw" "${PS_RAW}"
  !insertmacro PS_UNASSOC "srf" "${PS_RAW}"
  !insertmacro PS_UNASSOC "sr2" "${PS_RAW}"
  !insertmacro PS_UNASSOC "dng" "${PS_RAW}"
  !insertmacro PS_UNASSOC "orf" "${PS_RAW}"
  !insertmacro PS_UNASSOC "rw2" "${PS_RAW}"
  !insertmacro PS_UNASSOC "raf" "${PS_RAW}"
  DeleteRegKey SHCTX "Software\Classes\${PS_IMG}"
  DeleteRegKey SHCTX "Software\Classes\${PS_RAW}"
  DeleteRegKey SHCTX "Software\PhotoSee"
  DeleteRegValue SHCTX "Software\RegisteredApplications" "${PRODUCTNAME}"
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)"
!macroend
