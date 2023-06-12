# url-file-direct-uploader

Small library to read and upload file contents from a URL to an endpoint simultaneously. The received data chunks are directly sent to the upload endpoint without being saved to storage or memory first, making the process very fast and efficient compared to storing the file before uploading (limited by storage or memory size and the total duration is slower).

**Note: while this library might be useful for you, keep in mind that it was developed for learning purposes and requires more testing and improvements.**

___

## Usage

The library part is the main focus, but a simple `main.rs` is also provided for quick command line usage.

### As a library (flexible with access to more options)

```rust
use url_file_direct_uploader::{Manager, ManagerUploadOption};

fn main(){
        let manager = Manager::new(
            "DOWNLOAD_URL",
            "UPLOAD_URL",
            Some(vec![ // or None to use default values
                ManagerUploadOption::FileName(String::from("file.png")),
            ]),
        );

        // use the public functions of Manager here to customize

        manager.perform(); // or manager.perform_async()
}
```

### As a command line tool (limited to basic usage)

```
cargo run [DOWNLOAD_URL] [UPLOAD_URL] [?UPLOAD_FIELD_NAME] [?REQUEST_TYPE] [?UPLOAD_FILE_NAME]
```

Example:

```
cargo run "https://example.com/image.png" "https://example.com/upload" "file" "POST" "image.png"
```
___

## Alternative way using command line `curl`

It might be possible to achieve the same functionality of this library using two piped `curl` commands:

- `curl -s --output - "DOWNLOAD_URL"` to download
- `curl -F file=@- [...]`, `curl -T - [...]` or other options to upload

However:

- `curl -F file=@- [...]` was using a lot of memory as it was storing the data in memory before starting to upload (to determine the `Content-Length` for the upload headers)
- `curl -T - [...]` doesn't seem to have the option to specify the file field name, and is possibly not for HTTP uploads
- A possible solution is building the request body manually (like this library is doing) instead of relying on `curl` file arguments, and that might work with `-T -` or the other options
- In the case it's possible (in a similar buffering way), the memory usage was higher and the CPU usage was in close range (tested `curl -T - [...]` despite it not delivering the needed format)

___

## Things that are currently missing

- Cancelling the download request when the upload request couldn't be performed
- Automatically detecting the file name from the URL or download data and using it in the upload request when `ManagerUploadOption::FileName` is not provided
- Automatically detecting the `Content-Type` from the download headers and using it in the upload request when `ManagerUploadOption::ContentType` is not provided (having the `Content-Type` header is encouraged but generally not required)
- Better error reporting and handling
- Documentation