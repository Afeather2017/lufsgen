use std::env;
use std::path::Path;
use tracing_subscriber;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("LUFS Generator CLI");
    println!("==================\n");

    // Initialize FFmpeg
    if let Err(e) = lufsgen_android::init_ffmpeg() {
        eprintln!("Failed to initialize FFmpeg: {}", e);
        eprintln!("\nMake sure FFmpeg is installed on your system.");
        eprintln!("Download from: https://ffmpeg.org/download.html");
        std::process::exit(1);
    }

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: lufsgen <file-or-directory> [output-file]");
        println!("\nExamples:");
        println!("  lufsgen song.mp3");
        println!("  lufsgen ./music_folder");
        println!("  lufsgen . lufs_data.txt");
        std::process::exit(1);
    }

    let input = &args[1];
    let output = args.get(2).map(|s| s.as_str()).unwrap_or("lufs_data.txt");
    let path = Path::new(input);

    let results = if path.is_file() {
        let filename = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let lufs = match lufsgen_android::get_lufs(input) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Error processing {}: {}", input, e);
                vec![]
            }
        };

        vec![lufsgen_android::LufsResult {
            filename,
            path: input.to_string(),
            lufs,
        }]
    } else if path.is_dir() {
        lufsgen_android::scan_and_generate_lufs(input)
    } else {
        eprintln!("Error: Path does not exist: {}", input);
        std::process::exit(1);
    };

    // Display results
    for result in &results {
        match result.lufs {
            Some(lufs) => println!("{}: {:.2} LUFS", result.filename, lufs),
            None => println!("{}: FAILED", result.filename),
        }
    }

    // Write output
    if let Err(e) = lufsgen_android::write_lufs_data(&results, output) {
        eprintln!("Error writing to {}: {}", output, e);
    } else {
        println!("\nWritten {} entries to {}", results.len(), output);
    }
}
