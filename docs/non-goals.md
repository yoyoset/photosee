# Non-goals

PhotoSee shows pictures. Every feature request starts as "no". Something can
leave this list only if it replaces something else, or if the orchestrator
records here why it is worth its weight.

| Not doing | Why |
|---|---|
| Editing, cropping, filters, writing rotation back | Viewer, not editor; the user asked for none of it |
| Tags, albums, ratings, face recognition, search | Same |
| Print, share, upload | Same |
| Deleting, moving or renaming files | Destructive operations in a viewer cause irreversible accidents |
| WebDAV, Baidu/Aliyun drives, other cloud protocols | User picked SMB only. Synced folders already work as local paths |
| RAW development | Embedded previews are enough for looking at photos |
| Video | Not pictures |
| Translucent background | Costs WebView2 its GPU fast path; Tauri transparent regions swallow clicks |
| "Include subfolders" option | Overlaps with depth-first "next folder" |
| Configurable slideshow end folder | Implicit: the opened folder is the root |
| Language setting | Follows the system |
| PSB, ZIP-compressed PSD | Unsupported by zune-psd; rare in photo libraries |
| AVIF decoding in core | dav1d needs a C toolchain in every build; AVIF is passed through to the webview, so thumbnails of AVIF fall back to the original bytes |
| Plugins, themes, scripting | Growth trap |
| CBR, CB7, plain `.zip` as folders | CBR/CB7 need RAR/7z decoders with no reliable pure-Rust implementation; `.zip` next to photos is usually not a picture set. `.cbz` is supported (added 2026-11-05, the user reads comics from the NAS through the Docker build) |
| Comic-reader features: right-to-left paging, two-page spreads, reading progress, archive cover on folder tiles | Viewer, not a reader; a `.cbz` is just a folder of pages |
| ZIP64 `.cbz` (> 4 GiB or > 65535 pages), legacy code-page (GBK/Shift-JIS) member names | None of the 32 archives on the user's NAS needed either; ZIP64 is refused with `unsupported`, legacy names show lossy. Decoding GBK would need a new dependency |
| `.cbz` in the installer's "Open with" list | Opening an archive from Explorer needs start-up path handling for a file that is a folder; browse into it from the tree instead |
