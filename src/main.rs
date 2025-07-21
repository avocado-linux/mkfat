use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};

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
    input: Option<String>,
    output: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    files: Vec<FileEntry>,
    directories: Option<Vec<String>>,
}

fn generate_fat_image(cli: &Cli, manifest: &Manifest, base: &Path) -> io::Result<()> {
    // Create and preallocate output file
    let img_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&cli.output)?;
    img_file.set_len(cli.size_mb * 1024 * 1024)?;

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
    fatfs::format_volume(&mut boxed_file, format_options)?;

    // Rewind the file for filesystem operations
    boxed_file.seek(SeekFrom::Start(0))?;

    // Create filesystem
    let fs = FileSystem::new(boxed_file, FsOptions::new())?;
    let root_dir = fs.root_dir();

    if let Some(directories) = &manifest.directories {
        for dir_path in directories {
            if cli.verbose {
                println!("Creating directory: {}", dir_path);
            }
            let components_vec: Vec<_> = Path::new(dir_path).components().collect();
            let mut dir = root_dir.clone();
            for comp in &components_vec {
                let name = comp.as_os_str().to_str().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid UTF-8 in path"))?;
                dir = dir.create_dir(name).or_else(|_| dir.open_dir(name))?;
            }
        }
    }

    for entry in manifest.files.iter() {
        let input_path = entry.input.as_ref().unwrap_or_else(|| entry.output.as_ref().unwrap());
        let output_path = entry.output.as_ref().unwrap_or_else(|| entry.input.as_ref().unwrap());

        if cli.verbose {
            println!("Adding file: {} -> {}", input_path, output_path);
        }

        let full_input_path = base.join(input_path);
        let mut file_data = Vec::new();
        File::open(&full_input_path)?.read_to_end(&mut file_data)?;

        let components_vec: Vec<_> = Path::new(output_path).components().collect();
        let mut dir = root_dir.clone();

        for comp in &components_vec[..components_vec.len().saturating_sub(1)] {
            let name = comp.as_os_str().to_str().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid UTF-8 in path"))?;
            dir = dir.create_dir(name).or_else(|_| dir.open_dir(name))?;
        }

        let file_name = Path::new(output_path).file_name().and_then(|s| s.to_str()).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name"))?;
        let mut fat_file = dir.create_file(file_name)?;
        fat_file.write_all(&file_data)?;
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if !cli.quiet {
        println!("Reading manifest: {}", cli.manifest.display());
    }
    let json_str = fs::read_to_string(&cli.manifest)?;
    let manifest: Manifest = serde_json::from_str(&json_str)?;

    if !cli.quiet {
        println!("Generating FAT image: {}", cli.output.display());
    }
    generate_fat_image(&cli, &manifest, &cli.base)?;

    if !cli.quiet {
        println!("Done.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_fat_image() -> io::Result<()> {
        let tempdir = tempdir()?;
        let base_path = tempdir.path().join("base");
        let output_path = tempdir.path().join("image.fat");
        fs::create_dir(&base_path)?;

        let file_path = base_path.join("hello.txt");
        fs::write(&file_path, b"Hello, world!")?;

        let manifest = Manifest {
            files: vec![FileEntry {
                input: Some("hello.txt".to_string()),
                output: Some("greeting/hello.txt".to_string()),
            }],
            directories: None,
        };

        let cli = Cli {
            manifest: PathBuf::from("manifest.json"),
            base: base_path.clone(),
            output: output_path.clone(),
            size_mb: 4,
            label: "FATFS".to_string(),
            fat_type: FatType::Fat32,
            verbose: false,
            quiet: true,
        };

        generate_fat_image(&cli, &manifest, &base_path)?;

        assert!(output_path.exists());
        let metadata = fs::metadata(&output_path)?;
        assert!(metadata.len() >= 4 * 1024 * 1024);

        // Verify the contents of the image
        let img_file = File::open(&output_path)?;
        let fs = FileSystem::new(img_file, FsOptions::new())?;
        assert_eq!(fs.volume_label().trim_end(), "FATFS");
        let root_dir = fs.root_dir();
        let mut file = root_dir.open_file("greeting/hello.txt")?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        assert_eq!(contents, b"Hello, world!");
        Ok(())
    }

    #[test]
    fn test_generate_fat_image_no_output_path() -> io::Result<()> {
        let tempdir = tempdir()?;
        let base_path = tempdir.path().join("base");
        let output_path = tempdir.path().join("image.fat");
        fs::create_dir(&base_path)?;

        let file_path = base_path.join("another.txt");
        fs::write(&file_path, b"Another file content")?;

        let manifest = Manifest {
            files: vec![FileEntry {
                input: Some("another.txt".to_string()),
                output: None,
            }],
            directories: None,
        };

        let cli = Cli {
            manifest: PathBuf::from("manifest.json"),
            base: base_path.clone(),
            output: output_path.clone(),
            size_mb: 4,
            label: "FATFS".to_string(),
            fat_type: FatType::Fat32,
            verbose: false,
            quiet: true,
        };

        generate_fat_image(&cli, &manifest, &base_path)?;

        assert!(output_path.exists());

        // Verify the contents of the image
        let img_file = File::open(&output_path)?;
        let fs = FileSystem::new(img_file, FsOptions::new())?;
        let root_dir = fs.root_dir();
        let mut file = root_dir.open_file("another.txt")?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        assert_eq!(contents, b"Another file content");
        Ok(())
    }

    #[test]
    fn test_generate_fat_image_with_directories() -> io::Result<()> {
        let tempdir = tempdir()?;
        let base_path = tempdir.path().join("base");
        let output_path = tempdir.path().join("image.fat");
        fs::create_dir(&base_path)?;

        let manifest = Manifest {
            files: vec![],
            directories: Some(vec!["dir1".to_string(), "dir2/subdir".to_string()]),
        };

        let cli = Cli {
            manifest: PathBuf::from("manifest.json"),
            base: base_path.clone(),
            output: output_path.clone(),
            size_mb: 4,
            label: "FATFS".to_string(),
            fat_type: FatType::Fat32,
            verbose: false,
            quiet: true,
        };

        generate_fat_image(&cli, &manifest, &base_path)?;

        assert!(output_path.exists());

        // Verify the contents of the image
        let img_file = File::open(&output_path)?;
        let fs = FileSystem::new(img_file, FsOptions::new())?;
        let root_dir = fs.root_dir();
        assert!(root_dir.open_dir("dir1").is_ok());
        assert!(root_dir.open_dir("dir2/subdir").is_ok());
        Ok(())
    }
}
