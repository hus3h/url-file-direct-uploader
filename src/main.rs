use std::env;
use url_file_direct_uploader::{Manager, ManagerUploadOption, ManagerUploadRequestType};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 3 {
        let mut request_type = ManagerUploadRequestType::POST;
        let request_type_arg = args.get(4);
        if let Some(request_type_arg) = request_type_arg {
            let request_type_arg = &request_type_arg.trim().to_lowercase();
            if request_type_arg == "put" {
                request_type = ManagerUploadRequestType::PUT;
            } else if request_type_arg != "" && request_type_arg != "post" {
                println!("Warning: invalid request type provided, using POST (default)");
            }
        }

        let manager = Manager::new(
            args.get(1).unwrap(),
            args.get(2).unwrap(),
            Some(vec![
                ManagerUploadOption::FieldName(String::from(
                    args.get(3).unwrap_or(&String::from("file")),
                )),
                ManagerUploadOption::FileName(String::from(
                    args.get(5).unwrap_or(&String::from("file.bin")),
                )),
                ManagerUploadOption::RequestType(request_type),
            ]),
        );

        manager.perform();
    } else {
        println!(
            "Usage: {} [DOWNLOAD_URL] [UPLOAD_URL] [?UPLOAD_FIELD_NAME] [?REQUEST_TYPE] [?UPLOAD_FILE_NAME]",
            args.get(0).unwrap()
        )
    }
}
