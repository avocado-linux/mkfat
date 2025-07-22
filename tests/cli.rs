use std::fs;
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
                "input": "hello.txt",
                "output": "greeting/hello.txt"
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
