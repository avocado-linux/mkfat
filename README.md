# mkfat

A simple tool for creating FAT filesystem images from a JSON manifest.

## Usage

```sh
mkfat --manifest <manifest.json> --base <base_path> --output <image.fat>
```

## Options

| Option | Short | Description | Default |
| --- | --- | --- | --- |
| `--manifest` | `-m` | JSON file describing the files and directories to include | |
| `--base` | `-b` | Base path to find source files | |
| `--output` | `-o` | Output path for the generated FAT image | |
| `--size-mb` | `-s` | Size of the image in MB | 16 |
| `--label` | `-l` | Set the volume label | FATFS |
| `--fat-type` | | Set the FAT type (`fat12`, `fat16`, `fat32`) | `fat32` |
| `--verbose` | `-v` | Verbose output | |
| `--quiet` | `-q` | Quiet output | |

## Manifest Format

The manifest is a JSON file that describes the contents of the FAT image. It has two main keys: `files` and `directories`.

*   `files`: A list of file entries to include in the image. Each entry is an object with the following keys:
    *   `input`: The path to the source file, relative to the `base` path.
    *   `output`: The path where the file will be placed in the FAT image. If omitted, the `input` path is used.
*   `directories`: An optional list of empty directories to create in the image.

### Example Manifest

```json
{
  "files": [
    {
      "input": "data/hello.txt",
      "output": "greeting/hello.txt"
    },
    {
      "input": "data/another.txt"
    }
  ],
  "directories": [
    "empty_dir",
    "another_dir/subdir"
  ]
}
```

## Example

1.  Create a directory for your source files:

    ```sh
    mkdir -p my_files/data
    echo "Hello, world!" > my_files/data/hello.txt
    echo "Another file content" > my_files/data/another.txt
    ```

2.  Create a manifest file named `manifest.json`:

    ```json
    {
      "files": [
        {
          "input": "data/hello.txt",
          "output": "greeting/hello.txt"
        },
        {
          "input": "data/another.txt"
        }
      ],
      "directories": [
        "empty_dir"
      ]
    }
    ```

3.  Run `mkfat` to generate the FAT image:

    ```sh
    mkfat --manifest manifest.json --base my_files --output my_image.fat --size-mb 8 --label "MY_DISK"
    ```

This will create an 8MB FAT32 image named `my_image.fat` with the specified files and directories.
