# LUFS Generator for Android

Cross-compiled Rust binary that calculates LUFS (Loudness Units Full Scale) for audio files on Android using FFmpeg libraries.

## Prerequisites

- Android NDK 29.0.13846066 installed
- Rust toolchain
- FFmpeg source code in `./FFmpeg` directory
- MSYS2/MinGW for running shell scripts on Windows (optional)

## Build Steps

### Step 1: Build FFmpeg for Android ARM64

Run in Windows Command Prompt:
```cmd
cd E:\fa
build_ffmpeg_android.bat
```

Or in Git Bash/MSYS2:
```bash
cd /e/fa
./build_ffmpeg_android.sh
```

Output: `./install/arm64-v8a/lib/*.so` (FFmpeg shared libraries)

### Step 2: Build Rust Binary

```cmd
cargo build --target aarch64-linux-android --release
```

The binary will be at: `target/aarch64-linux-android/release/lufsgen`

### Step 3: Deploy to Android

```cmd
adb push target/aarch64-linux-android/release/lufsgen /data/local/tmp/
adb push install/arm64-v8a/lib/*.so /data/local/tmp/
adb shell chmod 755 /data/local/tmp/lufsgen
```

### Step 4: Run on Android

```cmd
# Calculate LUFS for a single file
adb shell /data/local/tmp/lufsgen /sdcard/music.mp3

# Scan a directory
adb shell /data/local/tmp/lufsgen /sdcard/Music

# Save results to file
adb shell "/data/local/tmp/lufsgen /sdcard/Music /sdcard/lufs_results.txt"
adb pull /sdcard/lufs_results.txt
```

## Usage

```
lufsgen <audio-file>          Calculate LUFS for a single file
lufsgen <directory>           Scan directory for audio files
lufsgen <file/dir> <output>   Save results to file
```

## Output Format

The binary outputs `filename|lufs` per line:
```
song.mp3|-12.5
track2.wav|-10.3
```

## Supported Audio Formats

- MP3 (`.mp3`)
- WAV (`.wav`)
- AAC (`.aac`, `.m4a`)
- OGG (`.ogg`)
- FLAC (`.flac`)

## Troubleshooting

### Build fails

Make sure FFmpeg libraries are built first:
```cmd
ls install/arm64-v8a/lib
```

Should show: `libavcodec.so`, `libavformat.so`, `libavutil.so`, `libavfilter.so`, `libswresample.so`, `libswscale.so`

### Runtime error: library not found

Make sure all FFmpeg `.so` files are in the same directory as the `lufsgen` binary on Android:
```cmd
adb shell ls /data/local/tmp/*.so
```

### Permission denied

Make the binary executable:
```cmd
adb shell chmod 755 /data/local/tmp/lufsgen
```

## Desktop Testing (Optional)

Install FFmpeg for Windows, then:
```cmd
cargo build --release
cargo run --release -- theme_of_seliana.mp3
```
