# LUFS Generator

A pure Rust library and CLI tool for calculating LUFS (Loudness Units Full Scale) of audio files. Uses streaming decoders to process audio chunk-by-chunk without loading entire files into memory, making it memory-efficient and suitable for any platform.

## Features

- **Cross-platform** - Pure Rust, runs on Linux, macOS, Windows, Android, and more
- **Streaming audio decoders** supporting MP3, OGG, WAV, FLAC, AAC, M4A, and MP4
- **Automatic format detection** from stream content (magic bytes) - no file extension required
- **Memory-efficient** chunk-based processing (8192 samples per chunk)
- **EBU R128 compliant** loudness measurement using ebur128 crate
- **Flexible input** - works with files, memory buffers, or any `Read + Seek` source

## Installation

### Cargo

```bash
cargo add lufsgen
```

### Build from source

```bash
cargo build --release
```

The binary will be at: `target/release/lufsgen`

## CLI Usage

```
lufsgen <audio-file>          Calculate LUFS for a single file
lufsgen <directory>           Scan directory for audio files
lufsgen <file/dir> <output>   Save results to file
```

### Examples

```bash
# Calculate LUFS for a single file
lufsgen song.mp3

# Scan a directory
lufsgen /path/to/Music

# Save results to file
lufsgen /path/to/Music output.txt
```

### Output Format

The binary outputs `filename|lufs` per line:
```
song.mp3|-12.5
track2.wav|-10.3
```

## Library Usage

### Basic usage with file paths

```rust
use lufsgen::LufsCalculator;
use std::path::Path;

let calc = LufsCalculator::default();

// From file path - format is auto-detected
let lufs = calc.calculate_from_file(Path::new("song.mp3"))?;
println!("LUFS: {}", lufs.unwrap());

// Supports many formats: mp3, ogg, wav, flac, aac, m4a, mp4
let lufs_flac = calc.calculate_from_file(Path::new("song.flac"))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Custom file access via trait

The library accepts any type implementing `Read + Seek`, allowing you to integrate with custom file systems, network streams, or platform-specific APIs:

```rust
use lufsgen::LufsCalculator;
use std::io::{Cursor, Read, Seek};

// Example 1: From memory (useful for embedded or mobile)
let audio_data = std::fs::read("song.mp3")?;
let cursor = Cursor::new(audio_data);
let calc = LufsCalculator::default();
let lufs = calc.calculate_from_reader(cursor)?;

// Example 2: Custom file system wrapper
struct MyFileWrapper {
    // Your custom file access implementation
}

impl Read for MyFileWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Your implementation
        Ok(0)
    }
}

impl Seek for MyFileWrapper {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        // Your implementation
        Ok(0)
    }
}

let my_file = MyFileWrapper { /* ... */ };
let calc = LufsCalculator::default();
let lufs = calc.calculate_from_reader(my_file)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Progress tracking

```rust
use lufsgen::LufsCalculator;
use std::sync::atomic::AtomicU64;
use std::path::Path;

let calc = LufsCalculator::default();
let progress = std::sync::Arc::new(AtomicU64::new(0));

// Spawn a thread to monitor progress
let progress_clone = progress.clone();
std::thread::spawn(move || {
    loop {
        let bytes = progress_clone.load(std::sync::atomic::Ordering::Relaxed);
        println!("Processed: {} bytes", bytes);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
});

let lufs = calc.calculate_from_file_with_progress(
    Path::new("large_file.flac"),
    Some(progress)
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Supported Audio Formats

| Extension | Format | Status |
|-----------|--------|--------|
| mp3 | MP3 | ✅ |
| ogg, oga | OGG/Vorbis | ✅ |
| wav | WAV | ✅ |
| flac | FLAC | ✅ |
| aac | AAC (ADTS) | ✅ |
| m4a, mp4 | MP4/AAC | ⚠️* |

*Some M4A/MP4 files may fail if the `moov` atom is at the end. See Troubleshooting below.

## How It Works

This project uses Symphonia, a pure Rust multimedia framework, to decode audio files:

1. **Auto-detect format** from file content (magic bytes), not extension
2. **Decode chunk-by-chunk** (8192 samples per chunk)
3. **Feed samples** to EBU R128 loudness analyzer
4. **Return** the integrated LUFS value

This streaming approach is memory-efficient and suitable for:
- Mobile devices (Android, iOS via Rust)
- Embedded systems
- Server-side batch processing
- Desktop applications

## Android Build

### Prerequisites

- Android NDK (tested with 23.2.8568313)
- Rust toolchain

### Step 1: Set environment variables

```bash
export NDK_HOME=/path/to/android-ndk
export CC_aarch64_linux_android=${NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=${NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang
```

### Step 2: Build for Android

```bash
cargo build --release --target aarch64-linux-android
```

The binary will be at: `target/aarch64-linux-android/release/lufsgen`

### Step 3: Deploy to Android

```bash
# Via SCP (e.g., Termux)
scp -P 8022 target/aarch64-linux-android/release/lufsgen 192.168.136.29:~/

# Or via ADB
adb push target/aarch64-linux-android/release/lufsgen /data/local/tmp/
adb shell chmod 755 /data/local/tmp/lufsgen
```

### Step 4: Run on Android

```bash
# Via SSH
ssh -p 8022 192.168.136.29 "~/lufsgen /sdcard/Music/song.mp3"

# Or via ADB shell
adb shell /data/local/tmp/lufsgen /sdcard/Music/song.mp3

# Scan directory
adb shell /data/local/tmp/lufsgen /sdcard/Music
```

## Troubleshooting

### Build fails

Make sure the NDK path is correct and the clang compiler exists:
```bash
ls ${NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android29-clang
```

### M4A/MP4 file fails with "Failed to detect audio format"

Some M4A files have metadata (`moov` atom) at the end of the file, which Symphonia cannot detect. Convert the file with FFmpeg to move metadata to the beginning:

```bash
ffmpeg -i input.m4a -c:a copy -movflags +faststart output.m4a
```

The `-c:a copy` flag copies the audio without re-encoding (fast, no quality loss). The `-movflags +faststart` flag optimizes for streaming by moving the `moov` atom to the front.

## License

MIT OR Apache-2.0
