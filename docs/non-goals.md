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
