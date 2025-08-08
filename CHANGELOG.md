# mkfat

## 0.4.0

* Enhancements
  * Changed `--fat-type` to `--variant`
  * Updated variants to `fat12|fat16|fat32`
  * Added support for parsing variant from `<manifest>.build_args.variant`
  * Moved the `files` key to `<manifest>.build_args.files`

## 0.3.0

* Enhancements
  * Files can be a string filename or an object with in / out
    A string is intrepreted as `{"in": "<string>"}`

## 0.2.0

* Enhancements
  * Added ability to take manifest over sdio
  * Changed "filename" to "in"
  * Changed "output" to "out"

## 0.1.1

* Enhanceents
  * Changed "input" to "filename"
