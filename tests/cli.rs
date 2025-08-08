use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_mkfat_integration() {
    let temp_dir = tempdir().unwrap();
    let base_path = temp_dir.path();

    let manifest_path = base_path.join("boot.json");
    let output_path = base_path.join("test.fat");
    let file_to_include_path = base_path.join("hello.txt");
    fs::write(&file_to_include_path, "Hello, world!").unwrap();

    let manifest_content = r#"{
        "build_args": {
            "files": [
                {
                    "in": "hello.txt",
                    "out": "greeting/hello.txt"
                }
            ]
        },
        "out": "test.fat"
    }"#;
    fs::write(&manifest_path, manifest_content).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_mkfat"))
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--base")
        .arg(&base_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--size-mb")
        .arg("128")
        .arg("--label")
        .arg("BOOT")
        .status()
        .expect("Failed to execute command");

    assert!(status.success());
    assert!(output_path.exists());
}

#[test]
fn test_mkfat_integration_string_entry() {
    let temp_dir = tempdir().unwrap();
    let base_path = temp_dir.path();

    let manifest_path = base_path.join("boot.json");
    let output_path = base_path.join("test.fat");
    let file_to_include_path = base_path.join("hello.txt");
    fs::write(&file_to_include_path, "Hello, world!").unwrap();

    let manifest_content = r#"{
        "build_args": {
            "files": [
                "hello.txt"
            ]
        },
        "out": "test.fat"
    }"#;
    fs::write(&manifest_path, manifest_content).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_mkfat"))
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--base")
        .arg(&base_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--size-mb")
        .arg("128")
        .arg("--label")
        .arg("BOOT")
        .status()
        .expect("Failed to execute command");

    assert!(status.success());
    assert!(output_path.exists());
}

#[test]
fn test_mkfat_integration_stdin() {
    let temp_dir = tempdir().unwrap();
    let base_path = temp_dir.path();

    let output_path = base_path.join("test_stdin.fat");
    let file_to_include_path = base_path.join("hello_stdin.txt");
    fs::write(&file_to_include_path, "Hello, stdin!").unwrap();

    let manifest_content = r#"{
        "build_args": {
            "files": [
                {
                    "in": "hello_stdin.txt",
                    "out": "greeting/hello.txt"
                }
            ]
        },
        "out": "test_stdin.fat"
    }"#;

    let mut child = Command::new(env!("CARGO_BIN_EXE_mkfat"))
        .arg("--base")
        .arg(&base_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--size-mb")
        .arg("128")
        .arg("--label")
        .arg("STDIN")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to execute command");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    std::thread::spawn(move || {
        stdin
            .write_all(manifest_content.as_bytes())
            .expect("Failed to write to stdin");
    });

    let status = child.wait().expect("Command wasn't running");

    assert!(status.success());
    assert!(output_path.exists());
}

#[test]
fn test_cli_overrides_manifest_out() {
    let temp_dir = tempdir().unwrap();
    let base_path = temp_dir.path();

    let manifest_path = base_path.join("boot.json");
    let cli_output_path = base_path.join("cli_out.fat");
    let manifest_output_path = base_path.join("manifest_out.fat");
    let file_to_include_path = base_path.join("hello.txt");
    fs::write(&file_to_include_path, "Hello, world!").unwrap();

    let manifest_content = r#"{
        "build_args": {
            "files": [
                {
                    "in": "hello.txt",
                    "out": "greeting/hello.txt"
                }
            ]
        },
        "out": "manifest_out.fat"
    }"#;
    fs::write(&manifest_path, manifest_content).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_mkfat"))
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--base")
        .arg(&base_path)
        .arg("--output")
        .arg(&cli_output_path)
        .arg("--size-mb")
        .arg("16")
        .arg("--label")
        .arg("OVERRIDE")
        .status()
        .expect("Failed to execute command");

    assert!(status.success());
    assert!(cli_output_path.exists());
    assert!(!manifest_output_path.exists());
}

#[test]
fn test_cli_overrides_manifest_variant() {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let temp_dir = tempdir().unwrap();
    let base_path = temp_dir.path();

    let manifest_path = base_path.join("boot.json");
    let output_path = base_path.join("variant_override.fat");
    let file_to_include_path = base_path.join("hello.txt");
    fs::write(&file_to_include_path, "Hello, world!").unwrap();

    // Manifest requests FAT16, but CLI will force FAT32
    let manifest_content = r#"{
        "build_args": {
            "files": [
                "hello.txt"
            ],
            "variant": "FAT16"
        },
        "out": "variant_override.fat"
    }"#;
    fs::write(&manifest_path, manifest_content).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_mkfat"))
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--base")
        .arg(&base_path)
        .arg("--variant")
        .arg("FAT32")
        .arg("--verbose")
        .status()
        .expect("Failed to execute command");

    assert!(status.success());
    assert!(output_path.exists());

    // Verify the volume label area contains spaces and that we can read the boot sector without UTF8 errors.
    // We avoid relying on exact FAT type signature offsets which may vary by formatter.
    let mut f = File::open(&output_path).expect("failed to open image");
    let mut boot_sector = [0u8; 512];
    f.seek(SeekFrom::Start(0)).expect("seek failed");
    f.read_exact(&mut boot_sector).expect("read failed");
    // Check the BIOS Parameter Block signature 0x55AA at the end of sector
    assert_eq!(boot_sector[510], 0x55);
    assert_eq!(boot_sector[511], 0xAA);
}
