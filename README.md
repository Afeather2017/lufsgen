# LUFS Generator for Android

Cross-compiled Rust binary that calculates LUFS (Loudness Units Full Scale) for audio files on Android using streaming audio decoders.

## Prerequisites

- Android NDK (tested with 23.2.8568313)
- Rust toolchain

## Build Steps

### Step 1: Set environment variables

```bash
export NDK_HOME=/path/to/android-ndk
export CC_aarch64_linux_android=${NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=${NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang
```

### Step 2: Build Rust Binary

```bash
cargo build --release --target aarch64-linux-android
```

The binary will be at: `target/aarch64-linux-android/release/lufsgen`

### Step 3: Deploy to Android

```bash
scp -P 8022 target/aarch64-linux-android/release/lufsgen 192.168.136.29:~/
# or with adb
adb push target/aarch64-linux-android/release/lufsgen /data/local/tmp/
adb shell chmod 755 /data/local/tmp/lufsgen
```

### Step 4: Run on Android

```bash
# Calculate LUFS for a single file
~/lufsgen /sdcard/music.mp3

# Scan a directory
~/lufsgen /sdcard/Music

# Save results to file
~/lufsgen /sdcard/Music /sdcard/lufs_results.txt
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
- AAC (`.aac`, `.m4a`, `.mp4`)
- OGG (`.ogg`, `.oga`)
- FLAC (`.flac`)

## How It Works

This project uses Symphonia, a pure Rust multimedia framework, to decode audio files. The streaming decoder:
1. Auto-detects audio format from file content (not just extension)
2. Decodes audio chunk-by-chunk (8192 samples per chunk)
3. Feeds samples to EBU R128 loudness analyzer
4. Returns the integrated LUFS value

This approach is memory-efficient and suitable for mobile devices.

## Desktop Testing

```bash
cargo build --release
cargo run --release -- /path/to/music.mp3
```

## Troubleshooting

### Build fails

Make sure the NDK path is correct and the clang compiler exists:
```bash
ls ${NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang
```

### Permission denied

Make the binary executable:
```bash
adb shell chmod 755 /data/local/tmp/lufsgen
```

### M4A/MP4 file fails with "Failed to detect audio format"

Some M4A files have metadata (`moov` atom) at the end of the file, which Symphonia cannot detect. Convert the file with FFmpeg to move metadata to the beginning:

```bash
ffmpeg -i input.m4a -c:a copy -movflags +faststart output.m4a
```

The `-c:a copy` flag copies the audio without re-encoding (fast, no quality loss). The `-movflags +faststart` flag optimizes for streaming by moving the `moov` atom to the front.

## Library Usage

This is also a library that can be used in other Rust projects:

```rust
use lufsgen::{LufsCalculator, is_audio_file};

// Check if file is supported
if is_audio_file("song.mp3") {
    // Calculate LUFS
    let lufs = LufsCalculator::new().calculate_from_file("song.mp3")?;
    println!("LUFS: {}", lufs);
}
```

See `lib.rs` for the full API documentation.
