use std::fs::{self, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf, Component};

use clap::Parser;
use fatfs::{FsOptions, FileSystem};
use serde::Deserialize;

// Custom trait that combines Read, Write, and Seek
trait ReadWriteSeek: Read + Write + Seek {}
impl<T: Read + Write + Seek> ReadWriteSeek for T {}

#[derive(Parser, Debug)]
#[command(name = "mkfat")]
#[command(about = "Create a FAT filesystem from a JSON description.")]
struct Cli {
    /// JSON file describing the files to include
    #[arg(short, long)]
    manifest: PathBuf,

    /// Base path to find source files
    #[arg(short, long)]
    base: PathBuf,

    /// Output path for the generated FAT image
    #[arg(short, long)]
    output: PathBuf,

    /// Size of the image in MB
    #[arg(short = 's', long, default_value_t = 16)]
    size_mb: u64,

    /// Set the volume label
    #[arg(short, long, default_value = "FATFS")]
    label: String,

    /// Set the FAT type
    #[arg(long, value_enum, default_value_t = FatType::Fat32)]
    fat_type: FatType,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Quiet output
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

#[derive(Debug, Deserialize)]
struct FileEntry {
    filename: Option<String>,
    output: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    files: Vec<FileEntry>,
    directories: Option<Vec<String>>,
}

fn generate_fat_image(cli: &Cli, manifest: &Manifest, base: &Path) -> Result<(), String> {
    // Create and preallocate output file
    let img_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&cli.output)
        .map_err(|e| {
            format!(
                "Failed to open output file '{}': {}",
                cli.output.display(),
                e
            )
        })?;
    img_file
        .set_len(cli.size_mb * 1024 * 1024)
        .map_err(|e| format!("Failed to set image size: {}", e))?;

    // Keep the file in a box to satisfy the 'static lifetime requirement
    let mut boxed_file: Box<dyn ReadWriteSeek> = Box::new(img_file);

    let fat_type = match cli.fat_type {
        FatType::Fat12 => fatfs::FatType::Fat12,
        FatType::Fat16 => fatfs::FatType::Fat16,
        FatType::Fat32 => fatfs::FatType::Fat32,
    };

    // Format the volume
    let mut label_bytes = [b' '; 11];
    label_bytes[..cli.label.len()].copy_from_slice(cli.label.as_bytes());
    let format_options = fatfs::FormatVolumeOptions::new()
        .volume_label(label_bytes)
        .fat_type(fat_type);
    fatfs::format_volume(&mut boxed_file, format_options)
        .map_err(|e| format!("Failed to format volume: {}", e))?;

    // Rewind the file for filesystem operations
    boxed_file
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("Failed to seek in image file: {}", e))?;

    // Create filesystem
    let fs = FileSystem::new(boxed_file, FsOptions::new())
        .map_err(|e| format!("Failed to create filesystem: {}", e))?;
    let root_dir = fs.root_dir();

    if let Some(directories) = &manifest.directories {
        for dir_path in directories {
            if cli.verbose {
                println!("Creating directory: {}", dir_path);
            }
            let components_vec: Vec<_> = Path::new(dir_path).components().collect();
            let mut dir = root_dir.clone();
            for comp in &components_vec {
                if let Component::RootDir = comp {
                    continue;
                }
                let name = comp
                    .as_os_str()
                    .to_str()
                    .ok_or("Invalid UTF-8 in path")?;
                dir = dir
                    .create_dir(name)
                    .or_else(|_| dir.open_dir(name))
                    .map_err(|e| format!("Failed to create directory '{}': {}", name, e))?;
            }
        }
    }

    for entry in manifest.files.iter() {
        let input_path = entry
            .filename
            .as_ref()
            .unwrap_or_else(|| entry.output.as_ref().unwrap());
        let output_path = entry
            .output
            .as_ref()
            .unwrap_or_else(|| entry.filename.as_ref().unwrap());

        if cli.verbose {
            println!("Adding file: {} -> {}", input_path, output_path);
        }

        let full_input_path = base.join(input_path);
        let file_data = fs::read(&full_input_path).map_err(|e| {
            format!(
                "Failed to read input file '{}': {}",
                full_input_path.display(),
                e
            )
        })?;

        let components_vec: Vec<_> = Path::new(output_path).components().collect();
        let mut dir = root_dir.clone();

        for comp in &components_vec[..components_vec.len().saturating_sub(1)] {
            if let Component::RootDir = comp {
                continue;
            }
            let name = comp
                .as_os_str()
                .to_str()
                .ok_or("Invalid UTF-8 in path")?;
            dir = dir
                .create_dir(name)
                .or_else(|_| dir.open_dir(name))
                .map_err(|e| format!("Failed to create directory '{}': {}", name, e))?;
        }

        let file_name = Path::new(output_path)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid file name")?;
        let mut fat_file = dir
            .create_file(file_name)
            .map_err(|e| format!("Failed to create file '{}': {}", file_name, e))?;
        fat_file
            .write_all(&file_data)
            .map_err(|e| format!("Failed to write to file '{}': {}", file_name, e))?;
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut cli = Cli::parse();

    if cli.base.is_relative() {
        cli.base = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?
            .join(&cli.base);
    }

    if !cli.quiet {
        println!("Reading manifest: {}", cli.manifest.display());
    }
    let json_str = fs::read_to_string(&cli.manifest).map_err(|e| {
        format!(
            "Failed to read manifest file '{}': {}",
            cli.manifest.display(),
            e
        )
    })?;
    let manifest: Manifest = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse manifest file: {}", e))?;

    if !cli.quiet {
        println!("Generating FAT image: {}", cli.output.display());
    }
    generate_fat_image(&cli, &manifest, &cli.base)?;

    if !cli.quiet {
        println!("Done.");
    }

    Ok(())
}
