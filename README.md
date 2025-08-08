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
| `--output` | `-o` | Output path for the generated FAT image. Overrides manifest `out`. | |
| `--size-mb` | `-s` | Size of the image in MB | 16 |
| `--label` | `-l` | Set the volume label | FATFS |
| `--variant` | | Set the filesystem variant (`FAT12`, `FAT16`, `FAT32`). Overrides manifest `build_args.variant`. | |
| `--verbose` | `-v` | Verbose output | |
| `--quiet` | `-q` | Quiet output | |

## Manifest Format

The manifest is a JSON file that describes the contents of the FAT image. Top-level keys:

*   `build_args`: Contains build-related configuration.
    *   `files`: A list of file entries to include in the image. Each entry can be either a string or an object with keys:
        *   `in`: The path to the source file, relative to the `base` path.
        *   `out`: The target path inside the FAT image. If omitted, the `in` path is used.
    *   `variant`: Optional string, one of `FAT12`, `FAT16`, `FAT32`. Maps to the filesystem type.
*   `out`: Optional filename for the generated image. If provided and `--output` is not used, the image will be written to `<base>/<out>`.
*   `directories`: An optional list of empty directories to create in the image.

### Example Manifest

```json
{
  "build_args": {
    "files": [
      {
        "in": "data/hello.txt",
        "out": "greeting/hello.txt"
      },
      {
        "in": "data/another.txt"
      }
    ]
  },
  "out": "my_image.fat",
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
      "build_args": {
        "files": [
          {
            "in": "data/hello.txt",
            "out": "greeting/hello.txt"
          },
          {
            "in": "data/another.txt"
          }
        ]
      },
      "out": "my_image.fat",
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
