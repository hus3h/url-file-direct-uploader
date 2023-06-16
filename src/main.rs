use std::env;
use url_file_direct_uploader::{Manager, ManagerUploadOption, ManagerUploadRequestType};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 3 {
        let mut request_type = ManagerUploadRequestType::POST;
        let request_type_arg = args.get(5);
        if let Some(request_type_arg) = request_type_arg {
            let request_type_arg = &request_type_arg.trim().to_lowercase();
            if request_type_arg == "put" {
                request_type = ManagerUploadRequestType::PUT;
            } else if request_type_arg != "" && request_type_arg != "post" {
                println!("Warning: invalid request type provided, using POST (default)");
            }
        }

        let mut options = vec![ManagerUploadOption::RequestType(request_type)];

        if let Some(value) = args.get(3) {
            if value.trim() != "" {
                options.push(ManagerUploadOption::FieldName(String::from(value)));
            }
        }

        if let Some(value) = args.get(4) {
            if value.trim() != "" {
                options.push(ManagerUploadOption::FileName(String::from(value)));
            }
        }

        let manager = Manager::new(
            args.get(1).unwrap(),
            args.get(2).unwrap(),
            Some(options),
            None,
        );

        manager.perform();
    } else {
        println!(
            "Usage: {} [DOWNLOAD_URL] [UPLOAD_URL] [?UPLOAD_FIELD_NAME] [?UPLOAD_FILE_NAME] [?REQUEST_TYPE]",
            args.get(0).unwrap()
        )
    }
}
