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
        "files": [
            {
                "in": "hello.txt",
                "out": "greeting/hello.txt"
            }
        ]
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
        "files": [
            "hello.txt"
        ]
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
        "files": [
            {
                "in": "hello_stdin.txt",
                "out": "greeting/hello.txt"
            }
        ]
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
