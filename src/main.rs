//! LUFS Generator for Android - Pure Rust
//! Calculates LUFS (Loudness Units Full Scale) for audio files
//! Uses individual decoder crates and ebur128 for loudness measurement
//!
//! Usage on Android:
//! adb push lufsgen /data/local/tmp/
//! adb shell chmod 755 /data/local/tmp/lufsgen
//! adb shell /data/local/tmp/lufsgen /sdcard/music.mp3

use std::env;
use std::path::Path;
use std::fs;
use std::io::BufReader;

/// Audio file extensions supported for LUFS analysis
const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "ogg"];

/// Represents LUFS analysis result
#[derive(Debug, Clone)]
struct LufsResult {
    filename: String,
    path: String,
    lufs: Option<f64>,
}

/// Calculate LUFS for a single audio file using pure Rust libraries
fn get_lufs(file_path: &Path) -> Result<Option<f64>, String> {
    // Check if file exists
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    let extension = file_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(None);
    }

    eprintln!("[LUFS] Processing: {}", file_path.display());

    // Decode based on extension
    let (sample_rate, channels, samples_i16): (u32, u32, Vec<i16>) = match extension.as_str() {
        "mp3" => decode_mp3(file_path)?,
        "ogg" => decode_ogg(file_path)?,
        "wav" => decode_wav(file_path)?,
        _ => return Ok(None),
    };

    // Initialize EBU R128 loudness meter
    let mut ebur = match ebur128::EbuR128::new(channels, sample_rate, ebur128::Mode::I) {
        Ok(e) => e,
        Err(e) => return Err(format!("Failed to create EBU R128: {:?}", e)),
    };

    // Convert i16 to f32 in range [-1.0, 1.0]
    let samples: Vec<f32> = samples_i16.iter()
        .map(|&s| s as f32 / 32768.0)
        .collect();

    // Feed to EBU R128
    if let Err(e) = ebur.add_frames_f32(&samples) {
        return Err(format!("EBU R128 processing error: {:?}", e));
    }

    // Get the loudness value
    let loudness = ebur.loudness_global()
        .map_err(|e| format!("Failed to get loudness: {:?}", e))?;

    eprintln!("[LUFS] {} - LUFS: {:.2}", file_path.display(), loudness);

    Ok(Some(loudness))
}

/// Decode MP3 using minimp3
fn decode_mp3(path: &Path) -> Result<(u32, u32, Vec<i16>), String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut decoder = minimp3::Decoder::new(&data[..]);
    let mut samples = Vec::new();
    let mut sample_rate = 44100; // default

    loop {
        match decoder.next_frame() {
            Ok(frame) => {
                sample_rate = frame.sample_rate as u32;
                samples.extend_from_slice(&frame.data);
            }
            Err(minimp3::Error::Eof) => break,
            Err(e) => return Err(format!("MP3 decode error: {:?}", e)),
        }
    }

    Ok((sample_rate, 2, samples)) // MP3 is typically stereo
}

/// Decode OGG/Vorbis using lewton
fn decode_ogg(path: &Path) -> Result<(u32, u32, Vec<i16>), String> {
    let file = fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut reader = BufReader::new(file);

    let mut decoder = lewton::inside_ogg::OggStreamReader::new(&mut reader)
        .map_err(|e| format!("Failed to create OGG reader: {}", e))?;

    let sample_rate = decoder.ident_hdr.audio_sample_rate;
    let channels = decoder.ident_hdr.audio_channels as u32;
    let mut samples = Vec::new();

    while let Ok(Some(packet)) = decoder.read_dec_packet_itl() {
        samples.extend(packet);
    }

    Ok((sample_rate, channels, samples))
}

/// Decode WAV using hound
fn decode_wav(path: &Path) -> Result<(u32, u32, Vec<i16>), String> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as u32;

    let samples: Vec<i16> = reader.into_samples()
        .filter_map(|s| s.ok())
        .collect();

    Ok((sample_rate, channels, samples))
}

/// Check if a file has supported audio extension
fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Scan directory recursively for audio files and generate LUFS data
fn scan_and_generate_lufs(root_dir: &Path) -> Vec<LufsResult> {
    eprintln!("Scanning: {}", root_dir.display());
    let mut results = Vec::new();

    let entries = match fs::read_dir(root_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to read directory: {}", e);
            return results;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() {
            if is_audio_file(&path) {
                let filename = path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let path_str = path.to_string_lossy().to_string();
                let lufs = get_lufs(&path).unwrap_or(None);

                results.push(LufsResult {
                    filename,
                    path: path_str,
                    lufs,
                });
            }
        } else if path.is_dir() {
            let mut sub_results = scan_and_generate_lufs(&path);
            results.append(&mut sub_results);
        }
    }

    results
}

fn print_usage() {
    println!("LUFS Generator for Android (Pure Rust)");
    println!();
    println!("Usage:");
    println!("  lufsgen <audio-file>          Calculate LUFS for a single file");
    println!("  lufsgen <directory>           Scan directory for audio files");
    println!("  lufsgen <file/dir> <output>   Save results to file");
    println!();
    println!("Examples:");
    println!("  adb shell /data/local/tmp/lufsgen /sdcard/music.mp3");
    println!("  adb shell /data/local/tmp/lufsgen /sdcard/Music");
    println!();
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let input = Path::new(&args[1]);

    let results = if input.is_file() {
        let filename = input.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let lufs = match get_lufs(input) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        };

        vec![LufsResult {
            filename,
            path: args[1].clone(),
            lufs,
        }]
    } else if input.is_dir() {
        scan_and_generate_lufs(input)
    } else {
        eprintln!("Error: Path does not exist: {}", input.display());
        std::process::exit(1);
    };

    // Output results to stdout
    for result in &results {
        match result.lufs {
            Some(lufs) => println!("{}|{}", result.filename, lufs),
            None => println!("{}|FAILED", result.filename),
        }
    }

    // Optionally write to file
    if args.len() >= 3 {
        let output_path = &args[2];
        if let Err(e) = write_lufs_data(&results, output_path) {
            eprintln!("Failed to write output: {}", e);
        }
    }

    // Exit code based on success
    let failed = results.iter().filter(|r| r.lufs.is_none()).count();
    if failed > 0 {
        std::process::exit(1);
    }
}

fn write_lufs_data(results: &[LufsResult], output_path: &str) -> std::io::Result<()> {
    let mut content = String::new();

    for result in results {
        if let Some(lufs) = result.lufs {
            content.push_str(&format!("{}: {:.2} LUFS\n", result.filename, lufs));
        }
    }

    fs::write(output_path, content)
}
