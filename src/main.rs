use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use clap::Parser;
use fatfs::{FileSystem, FsOptions};
use serde::Deserialize;

// Custom trait that combines Read, Write, and Seek
trait ReadWriteSeek: Read + Write + Seek {}
impl<T: Read + Write + Seek> ReadWriteSeek for T {}

#[derive(Parser, Debug)]
#[command(name = "mkfat")]
#[command(about = "Create a FAT filesystem from a JSON description.")]
struct Cli {
    /// JSON file describing the files to include. If not provided, reads from stdin.
    #[arg(short, long)]
    manifest: Option<PathBuf>,

    /// Base path to find source files
    #[arg(short, long)]
    base: PathBuf,

    /// Output path for the generated FAT image (overrides manifest top-level 'out')
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Size of the image in MB
    #[arg(short = 's', long, default_value_t = 16)]
    size_mb: u64,

    /// Set the volume label
    #[arg(short, long, default_value = "FATFS")]
    label: String,

    /// Set the filesystem variant (overrides manifest build_args.variant)
    #[arg(long, value_enum)]
    variant: Option<ManifestVariant>,

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

#[derive(Debug)]
struct FileEntry {
    r#in: String,
    out: Option<String>,
}

impl FileEntry {
    fn get_in(&self) -> &str {
        &self.r#in
    }

    fn get_out(&self) -> &str {
        self.out.as_deref().unwrap_or_else(|| self.get_in())
    }
}

impl<'de> Deserialize<'de> for FileEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FileEntryVisitor;

        impl<'de> serde::de::Visitor<'de> for FileEntryVisitor {
            type Value = FileEntry;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or an object with 'in' and 'out' keys")
            }

            fn visit_str<E>(self, value: &str) -> Result<FileEntry, E>
            where
                E: serde::de::Error,
            {
                Ok(FileEntry {
                    r#in: value.to_string(),
                    out: None,
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<FileEntry, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut r#in: Option<String> = None;
                let mut out: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "in" => {
                            if r#in.is_some() {
                                return Err(serde::de::Error::duplicate_field("in"));
                            }
                            r#in = Some(map.next_value()?);
                        }
                        "out" => {
                            if out.is_some() {
                                return Err(serde::de::Error::duplicate_field("out"));
                            }
                            out = Some(map.next_value()?);
                        }
                        _ => {
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }

                let r#in = r#in.ok_or_else(|| serde::de::Error::missing_field("in"))?;
                Ok(FileEntry { r#in, out })
            }
        }

        deserializer.deserialize_any(FileEntryVisitor)
    }
}

#[derive(Debug, Deserialize, clap::ValueEnum, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
#[value(rename_all = "UPPERCASE")]
enum ManifestVariant {
    FAT12,
    FAT16,
    FAT32,
}

#[derive(Debug, Deserialize)]
struct BuildArgs {
    files: Vec<FileEntry>,
    variant: Option<ManifestVariant>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    build_args: BuildArgs,
    directories: Option<Vec<String>>,
    /// Optional output filename; when present and CLI --output not provided,
    /// the effective output path will be base directory joined with this filename
    out: Option<String>,
}

fn generate_fat_image(
    cli: &Cli,
    manifest: &Manifest,
    base: &Path,
    effective_fat_type: FatType,
    output_path: &Path,
) -> Result<(), String> {
    // Create and preallocate output file
    let img_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .map_err(|e| {
            format!(
                "Failed to open output file '{}': {}",
                output_path.display(),
                e
            )
        })?;
    img_file
        .set_len(cli.size_mb * 1024 * 1024)
        .map_err(|e| format!("Failed to set image size: {}", e))?;

    // Keep the file in a box to satisfy the 'static lifetime requirement
    let mut boxed_file: Box<dyn ReadWriteSeek> = Box::new(img_file);

    let fat_type = match effective_fat_type {
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
                let name = comp.as_os_str().to_str().ok_or("Invalid UTF-8 in path")?;
                dir = dir
                    .create_dir(name)
                    .or_else(|_| dir.open_dir(name))
                    .map_err(|e| format!("Failed to create directory '{}': {}", name, e))?;
            }
        }
    }

    for entry in manifest.build_args.files.iter() {
        let input_path = entry.get_in();
        let output_path = entry.get_out();

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
            let name = comp.as_os_str().to_str().ok_or("Invalid UTF-8 in path")?;
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

    let json_str = if let Some(manifest_path) = &cli.manifest {
        if !cli.quiet {
            println!("Reading manifest: {}", manifest_path.display());
        }
        fs::read_to_string(manifest_path).map_err(|e| {
            format!(
                "Failed to read manifest file '{}': {}",
                manifest_path.display(),
                e
            )
        })?
    } else {
        if !cli.quiet {
            println!("Reading manifest from stdin");
        }
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| format!("Failed to read from stdin: {}", e))?;
        buffer
    };
    let manifest: Manifest = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse manifest file: {}", e))?;

    // Determine effective FAT type: CLI overrides manifest, else default to FAT32
    let effective_fat_type = if let Some(cli_variant) = cli.variant {
        match cli_variant {
            ManifestVariant::FAT12 => FatType::Fat12,
            ManifestVariant::FAT16 => FatType::Fat16,
            ManifestVariant::FAT32 => FatType::Fat32,
        }
    } else if let Some(variant) = &manifest.build_args.variant {
        match variant {
            ManifestVariant::FAT12 => FatType::Fat12,
            ManifestVariant::FAT16 => FatType::Fat16,
            ManifestVariant::FAT32 => FatType::Fat32,
        }
    } else {
        FatType::Fat32
    };

    // Determine effective output path: CLI overrides manifest 'out'
    let effective_output_path = if let Some(cli_out) = &cli.output {
        cli_out.clone()
    } else if let Some(out_name) = &manifest.out {
        cli.base.join(out_name)
    } else {
        return Err("Output path not specified. Provide --output or 'out' in manifest.".to_string());
    };

    if !cli.quiet {
        println!("Generating FAT image: {}", effective_output_path.display());
        if cli.verbose {
            let fat_type_str = match effective_fat_type {
                FatType::Fat12 => "fat12",
                FatType::Fat16 => "fat16",
                FatType::Fat32 => "fat32",
            };
            println!("FAT type: {}", fat_type_str);
        }
    }

    generate_fat_image(
        &cli,
        &manifest,
        &cli.base,
        effective_fat_type,
        &effective_output_path,
    )?;

    if !cli.quiet {
        println!("Done.");
    }

    Ok(())
}
