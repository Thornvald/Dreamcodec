# FFmpeg Binary (Optional)

This directory is for optionally bundling FFmpeg with the application.

## How FFmpeg is Found

The application uses automatic FFmpeg detection in this priority order:

1. **Bundled** - Same directory as the application executable
2. **System PATH** - Runs `where ffmpeg` to find it
3. **Common locations**:
   - `C:\ffmpeg\bin\ffmpeg.exe`
   - `C:\Program Files\ffmpeg\bin\ffmpeg.exe`
   - `C:\ffmpeg\ffmpeg.exe`
   - Chocolatey: `C:\ProgramData\chocolatey\bin\ffmpeg.exe`
   - `C:\tools\ffmpeg\bin\ffmpeg.exe`
4. **Windows Package Manager** - `%LOCALAPPDATA%\Microsoft\WinGet\Packages\*`
5. **App data directory** - Downloaded FFmpeg for this app

## Bundling FFmpeg (Optional)

To bundle FFmpeg with the installer:

1. Download FFmpeg for Windows from: https://www.gyan.dev/ffmpeg/builds/
   - Download the `ffmpeg-release-essentials.zip`

2. Extract and find `ffmpeg.exe`

3. Rename it to: `ffmpeg-x86_64-pc-windows-msvc.exe`
   - This is Tauri's expected naming convention for external binaries

4. Place it in this directory

5. Update `tauri.conf.json`:
   ```json
   "bundle": {
     "externalBin": ["ffmpeg"]
   }
   ```

## Auto-Download

If FFmpeg is not found anywhere, the app will automatically download it on first run and store it in:
- `%APPDATA%\Dreamcodec\ffmpeg.exe`

The user never needs to manually select FFmpeg.
