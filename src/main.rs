//! LUFS Generator - Pure Rust
//! Calculates LUFS (Loudness Units Full Scale) for audio files
//!
//! Cross-platform binary that works on Linux, macOS, Windows, Android, and more.
//!
//! Usage:
//!   lufsgen song.mp3              # Single file
//!   lufsgen /path/to/Music        # Directory scan

use std::env;
use std::path::Path;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

// Use the library
use lufsgen::{LufsCalculator, LufsResult, is_audio_file};

/// Process a single file and return its result
fn process_single_file(path: &Path) -> LufsResult {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let path_str = path.to_string_lossy().to_string();

    // Get file size for progress bar
    let file_size = fs::metadata(path)
        .ok()
        .map(|m| m.len())
        .unwrap_or(0);

    // Show file info
    if file_size > 1024 * 1024 {
        eprintln!("[LUFS] Processing: {} ({:.1} MB)", filename, file_size as f64 / 1024.0 / 1024.0);
    } else {
        eprintln!("[LUFS] Processing: {}", filename);
    }

    // Set up progress tracking for files larger than 1MB
    let progress = if file_size > 1024 * 1024 {
        Some(Arc::new(AtomicU64::new(0)))
    } else {
        None
    };

    // Spawn progress bar thread if we have progress tracking
    let progress_clone = progress.clone();
    let handle = if progress.is_some() {
        let filename_clone = filename.clone();
        Some(thread::spawn(move || {
            show_file_progress(&filename_clone, file_size, progress_clone.unwrap());
        }))
    } else {
        None
    };

    let calc = LufsCalculator::default();
    let lufs = match calc.calculate_from_file_with_progress(path, progress) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error processing {}: {}", filename, e);
            None
        }
    };

    // Wait for progress thread to finish
    if let Some(h) = handle {
        let _ = h.join();
        eprintln!(); // New line after progress bar
    }

    LufsResult { filename, path: path_str, lufs }
}

/// Show progress bar for a single file
fn show_file_progress(filename: &str, total_size: u64, progress: Arc<AtomicU64>) {
    use indicatif::{ProgressBar, ProgressStyle};

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({percent}%) {msg}")
            .expect("invalid template")
            .progress_chars("##-")
    );
    pb.set_message(filename.to_string());

    let mut last_bytes = 0;
    loop {
        let bytes_read = progress.load(Ordering::Relaxed);

        if bytes_read >= total_size {
            pb.finish();
            break;
        }

        pb.set_position(bytes_read);

        // Check if we're making progress
        if bytes_read == last_bytes {
            // No progress for 100ms - might be stuck or finished
            thread::sleep(Duration::from_millis(100));
        } else {
            last_bytes = bytes_read;
            thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Recursively scan a directory for audio files and process them
fn process_directory(root_dir: &Path) -> Vec<LufsResult> {
    eprintln!("Scanning: {}", root_dir.display());

    // First, collect all audio files
    let mut audio_files = Vec::new();
    collect_audio_files(root_dir, &mut audio_files);

    let total = audio_files.len();
    if total == 0 {
        eprintln!("No audio files found.");
        return Vec::new();
    }

    eprintln!("Found {} audio file(s)", total);

    // Set up progress bar
    use indicatif::{ProgressBar, ProgressStyle};
    let progress = ProgressBar::new(total as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .expect("invalid template")
            .progress_chars("##-")
    );

    let mut results = Vec::new();

    for path in audio_files {
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        progress.set_message(filename.clone());

        let calc = LufsCalculator::default();
        let lufs = match calc.calculate_from_file(&path) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("\nError: {}", e);
                None
            }
        };

        let path_str = path.to_string_lossy().to_string();
        results.push(LufsResult { filename, path: path_str, lufs });

        progress.inc(1);
    }

    progress.finish();
    eprintln!(); // New line after progress bar

    results
}

/// Collect all audio files recursively
fn collect_audio_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_audio_file(&path) {
                files.push(path);
            } else if path.is_dir() {
                collect_audio_files(&path, files);
            }
        }
    }
}

/// Output results to stdout
fn output_results(results: &[LufsResult]) {
    for result in results {
        match result.lufs {
            Some(lufs) => println!("{}|{}", result.filename, lufs),
            None => println!("{}|FAILED", result.filename),
        }
    }
}

/// Write results to file
fn write_results(results: &[LufsResult], output_path: &str) -> std::io::Result<()> {
    let mut content = String::new();

    for result in results {
        if let Some(lufs) = result.lufs {
            content.push_str(&format!("{}: {:.2} LUFS\n", result.filename, lufs));
        }
    }

    fs::write(output_path, content)
}

/// Print usage information
fn print_usage() {
    println!("LUFS Generator (Pure Rust)");
    println!();
    println!("Usage:");
    println!("  lufsgen <audio-file>          Calculate LUFS for a single file");
    println!("  lufsgen <directory>           Scan directory for audio files");
    println!("  lufsgen <file/dir> <output>   Save results to file");
    println!();
    println!("Examples:");
    println!("  lufsgen song.mp3");
    println!("  lufsgen ~/Music");
    println!("  # On Android:");
    println!("  adb shell /data/local/tmp/lufsgen /sdcard/music.mp3");
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
        vec![process_single_file(input)]
    } else if input.is_dir() {
        process_directory(input)
    } else {
        eprintln!("Error: Path does not exist: {}", input.display());
        std::process::exit(1);
    };

    // Output results to stdout
    output_results(&results);

    // Optionally write to file
    if args.len() >= 3 {
        let output_path = &args[2];
        if let Err(e) = write_results(&results, output_path) {
            eprintln!("Failed to write output: {}", e);
        }
    }

    // Exit code based on success
    let failed = results.iter().filter(|r| r.lufs.is_none()).count();
    if failed > 0 {
        std::process::exit(1);
    }
}
