# Changelog

## 0.2.1 — 2026-11-05

Bug-fix release for phones, based on [issue #1](https://github.com/TheHolyOneZ/ZWormHole/issues/1). Thanks to @cclafeer for the report and the screen recording.

### Fixes — Android & iOS

- **The status bar was invisible when the phone was in light mode.** Android drew dark icons on ZWormHole's dark background, so the clock, battery and signal seemed to be gone. The status and navigation bars now always use light icons. iPhones had the same problem, and ZWormHole now always uses the dark system style there.
- **White squares appeared at the edges of the screen.** They were unstyled scrollbar corners. They're now transparent, and the sideways scrolling that caused them is gone.
- **Dashboard and Focus could be dragged sideways.** A decorative background glow and the hidden encryption popup were both wider than a phone screen, which added extra horizontal scrollbars and a second vertical one. Both now stay within the screen.
- **The Focus header overlapped.** *Encrypted* ran into the benchmark and speed-test buttons on narrow screens. The buttons now have their own full-width row, which also makes them easier to tap.
- **Settings rows were squeezed.** *Received folders* had its description crushed to a few letters behind the dropdown, and other rows wrapped badly. Controls now move below the text when there isn't room.
- **Very small screens.** The Dashboard stats, the folder picker toolbar, the benchmark controls and the *This device* dialog no longer run off the edge on small phones or with larger text. Checked at screen sizes from 280px wide up, matching small Android phones and iPhones from the SE to the 14, in portrait and landscape.
- **Dialogs could open scrolled down.** On short screens the incoming-transfer, pairing and connection-test dialogs jumped to the Accept button, which hid who was asking. They now open at the top, so you always see the sender first.
- **Dialogs on older phones** now stay within the screen on system WebViews and iOS versions that don't support newer CSS units.

### Fixes — Desktop

- The same glow and popup fixes apply to small desktop windows, which could also show a sideways scrollbar.

## 0.2.0 — 2026-10-31

ZWormHole now runs on **Android** and **iPhone/iPad** — same app, same code base, same encrypted Noise-over-QUIC protocol. A phone pairs and transfers with Windows, macOS and Linux exactly like any other device.

### New — Android & iOS

- **One app, five platforms.** The Tauri app now builds for Windows, macOS, Linux, Android (arm64 + armv7 APK) and iOS (unsigned IPA for sideloading).
- **Native mobile bridge** (`tauri-plugin-zwormhole-mobile`, Kotlin + Swift) bundled in the repo, giving phones the full desktop feature set:
  - **Send files** with the system picker. On iOS you choose between *Photos & Videos* and *Files*.
  - **Send folders.** Pick a folder, choose files in the folder tree, then send it the same way as on desktop.
  - **Send clipboard images** from the phone's clipboard.
  - **Open and show received files.** Files open in the matching app, and *Show in folder* opens the Files app.
  - **Custom save folder.** On Android any folder on internal storage or the SD card works. On iOS any folder in Files works, including iCloud Drive, and the choice is remembered across launches.
  - **Export history and benchmarks** through the system "Save to…" sheet.
  - **Profile avatars** picked from the photo library. iOS converts HEIC to JPEG automatically.
- **Discovery on phones.**
  - Android keeps a Wi-Fi multicast lock so mDNS discovery works on every network that allows it.
  - iOS uses native Bonjour (`NetService`). Sideloaded apps can't use raw multicast, so this keeps nearby discovery working without special entitlements. Desktop and Android see iPhones, and iPhones see them.
- **Received files land where you'd expect.** On Android they go to the public **Download** folder, are indexed so they show up in the Files/Downloads app right away, and on Android 9 and older ZWormHole asks for storage access. On iOS they go to **Files › On My iPhone › ZWormHole**.
- **Background receiving on Android.** *Run in background* starts a foreground service with a quiet notification, so paired devices can still reach you after you leave the app. On iOS, ongoing transfers get the short grace period iOS allows.
- **Notifications** for finished transfers and incoming text. Mobile asks for notification permission on first launch.
- **Real device names** on phones: the Android device name (or maker and model) and the iOS device name, instead of `localhost`.
- **Device badges** now show *Android* and *iOS* next to *Windows*, *macOS* and *Linux*.

### New — Mobile interface

- Phone layout with a bottom tab bar (**Devices · Dashboard · Focus · Settings**). The sidebar becomes its own *Devices* tab.
- Respects notches, the Dynamic Island and the home indicator (safe-area insets).
- Hides window controls, the drag-and-drop overlay and keyboard-shortcut hints on phones. Shows touch-friendly hints and larger tap targets instead.
- Long-press opens device and transfer context menus on iOS, matching Android and desktop right-click.
- Inputs use a 16px font on phones, so iOS no longer zooms the page when you tap a field.
- Modals, the downloads tray and toasts fit narrow screens.

### Build & release

- The GitHub Actions workflow now also builds:
  - `ZWormHole_<version>_android.apk`, signed with the `ANDROID_KEYSTORE_B64` / `ANDROID_KEY_PASSWORD` (and optional `ANDROID_KEY_ALIAS`) secrets. Without them it falls back to a throwaway CI key and warns.
  - `ZWormHole_<version>_ios_unsigned.ipa`, unsigned and ready for SideStore/AltStore.

  Both are published to the release next to the desktop installers and included in `SHA256SUMS.txt`.
- New helper scripts: `scripts/android-setup.mjs` (release signing) and `scripts/ios-unsigned.py` (removes code signing from the generated Xcode project).
- New `pnpm typecheck` and `pnpm android:setup` scripts.
- The workflow's path filter now points at the real workflow file and includes `scripts/**`.

### Fixes

- **About showed a hard-coded version number.** It now reads the real app version, so it can't fall out of date again.
- **"Run in background" survived only until restart.** The setting is now saved and restored on launch, on every platform.
- **Errors showed as `[object Object]`** in history, connection-test, incoming-transfer and device-info messages. They now show the real reason.
- **Avatars in WebP, GIF or BMP failed to load** even though the picker offered them. These formats now decode correctly.
- **The text composer showed `⌘ ↵` on Windows and Linux.** It now shows `Ctrl ↵` there.
- **Picking a save folder you can't write to** used to be accepted silently, and transfers failed later. It's now checked and rejected right away with a clear message.
- **Picker errors** in Settings, the incoming-transfer dialog and multi-send are now shown as toasts instead of failing silently.
- **Automatic port forwarding (NAT-PMP)** now falls back to the usual `x.x.x.1` gateway when the system routing table can't be read (Android, sandboxed environments).
