use curl::easy::{Easy, List};
use std::{
    collections::HashMap,
    io::Read,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

#[derive(Debug)]
enum ManagerThreadMessage {
    Bytes(Box<[u8]>),
    Header(String),
    EndOfHeadersSignal,
    PrepareToFinishSignal,
    StopSignal,
}

#[derive(Clone, Copy)]
pub enum ManagerUploadRequestType {
    POST,
    PUT,
}

#[derive(Clone)]
pub enum ManagerUploadOption {
    FieldName(String),
    FileName(String),
    ContentType(String),
    RequestType(ManagerUploadRequestType),
}

struct DownloadManager {
    download_url: String,
    download_headers: Vec<String>,
    message_sender: Arc<Mutex<Sender<ManagerThreadMessage>>>,
}

struct UploadManager {
    download_url: String,
    upload_url: String,
    upload_headers: Vec<String>,
    upload_options: Vec<ManagerUploadOption>,
    upload_form_fields: HashMap<String, String>,
    http_boundary: String,
    message_receiver: Arc<Mutex<Receiver<ManagerThreadMessage>>>,
}

impl DownloadManager {
    fn perform(&self) -> Result<(), curl::Error> {
        let mut request = Easy::new();
        request.url(&self.download_url).unwrap();
        request
            .http_headers(vec_to_easy_list(&self.download_headers))
            .unwrap();

        let header_sender = Arc::clone(&self.message_sender);
        let body_sender = Arc::clone(&self.message_sender);
        let final_sender = Arc::clone(&self.message_sender);

        request
            .header_function(move |mut data| {
                let mut header_data = String::with_capacity(data.len());
                data.read_to_string(&mut header_data).unwrap();

                if header_data != "\r\n" {
                    header_sender
                        .lock()
                        .unwrap()
                        .send(ManagerThreadMessage::Header(header_data))
                        .unwrap();
                } else {
                    header_sender
                        .lock()
                        .unwrap()
                        .send(ManagerThreadMessage::EndOfHeadersSignal)
                        .unwrap();
                }

                true
            })
            .unwrap();

        request
            .write_function(move |data| {
                let mut copied_data = vec![0; data.len()];
                copied_data.copy_from_slice(data);
                body_sender
                    .lock()
                    .unwrap()
                    .send(ManagerThreadMessage::Bytes(copied_data.into_boxed_slice()))
                    .unwrap();
                Ok(data.len())
            })
            .unwrap();

        let result = request.perform();

        final_sender
            .lock()
            .unwrap()
            .send(ManagerThreadMessage::PrepareToFinishSignal)
            .unwrap();

        final_sender
            .lock()
            .unwrap()
            .send(ManagerThreadMessage::StopSignal)
            .unwrap();

        result
    }

    fn set_headers(&mut self, headers: &Vec<&str>) {
        self.download_headers = headers.iter().map(|item| (*item).to_owned()).collect();
    }
}

impl UploadManager {
    fn perform(&self) -> thread::JoinHandle<()> {
        let initial_receiver = Arc::clone(&self.message_receiver);
        let download_url = self.download_url.to_owned();
        let upload_url = self.upload_url.to_owned();
        let http_boundary = self.http_boundary.to_owned();
        let mut headers = self.upload_headers.clone();
        let upload_options = self.upload_options.clone();
        let form_fields = self.upload_form_fields.clone();

        let request_thread = thread::spawn(move || {
            let receiver = Arc::clone(&initial_receiver);
            let mut request_type = ManagerUploadRequestType::POST;
            let mut content_type = None;
            let mut field_name = String::from("file");
            let mut file_name = String::from("");

            loop {
                let message = initial_receiver.lock().unwrap().recv().unwrap();

                match message {
                    ManagerThreadMessage::Header(value) => {
                        if let Some((key, value)) = get_header_string_value(value) {
                            if key.to_lowercase() == "content-type" {
                                content_type = Some(value);
                            }
                        }
                    }
                    ManagerThreadMessage::EndOfHeadersSignal => break,
                    _ => panic!("Received an unexpected signal"),
                }
            }

            for option in upload_options {
                match option {
                    ManagerUploadOption::FieldName(value) => field_name = value,
                    ManagerUploadOption::FileName(value) => file_name = value,
                    ManagerUploadOption::ContentType(value) => content_type = Some(value),
                    ManagerUploadOption::RequestType(value) => request_type = value,
                }
            }

            if file_name == "" {
                file_name = guess_file_name_from_url(&download_url);
            }

            headers.push(format!(
                "Content-Type: multipart/form-data; boundary={http_boundary}"
            ));

            let mut content_type_line = String::new();
            if let Some(value) = content_type {
                content_type_line = format!("\nContent-Type: {value}");
            }

            let mut form_fields_data = String::new();
            for (field_name, field_value) in form_fields {
                form_fields_data = form_fields_data + &format!("--{http_boundary}\r\nContent-Disposition: form-data; name=\"{field_name}\"\r\n\r\n{field_value}\r\n");
            }

            let mut initial_data = Some(format!("{form_fields_data}--{http_boundary}\r\nContent-Disposition: form-data; name=\"{field_name}\"; filename=\"{file_name}\"{content_type_line}\r\n\r\n"));

            let final_data = format!("\r\n--{http_boundary}--\r\n");

            let mut request = Easy::new();
            request.url(&upload_url).unwrap();
            request.http_headers(vec_to_easy_list(&headers)).unwrap();

            match request_type {
                ManagerUploadRequestType::POST => request.post(true).unwrap(),
                ManagerUploadRequestType::PUT => request.put(true).unwrap(),
            }

            request
                .read_function(move |buf| {
                    let message = receiver.lock().unwrap().recv().unwrap();

                    if let Some(initial_data_string) = &initial_data {
                        if let ManagerThreadMessage::Bytes(first_message) = message {
                            let initial_data_bytes = initial_data_string.as_bytes();
                            let first_message_bytes = first_message.as_ref();

                            let mut final_data = initial_data_bytes.to_vec();
                            final_data.append(&mut first_message_bytes.to_vec());

                            let result = final_data.into_boxed_slice().as_ref().read(buf).unwrap();

                            initial_data = None;

                            Ok(result)
                        } else {
                            panic!("Received wrong first message");
                        }
                    } else if let ManagerThreadMessage::Bytes(data) = message {
                        Ok(data.as_ref().read(buf).unwrap())
                    } else if let ManagerThreadMessage::PrepareToFinishSignal = message {
                        Ok(final_data.as_bytes().read(buf).unwrap())
                    } else if let ManagerThreadMessage::StopSignal = message {
                        Ok(0)
                    } else {
                        panic!("Received wrong message");
                    }
                })
                .unwrap();

            request
                .write_function(|data| {
                    let mut buf = String::new();
                    data.clone().read_to_string(&mut buf).unwrap();
                    println!("{}", buf);
                    Ok(data.len())
                })
                .unwrap();

            println!("[i] Starting to receive and send data, server response will be printed");

            request.perform().unwrap();
        });

        request_thread
    }

    fn set_upload_options(&mut self, options: &Vec<ManagerUploadOption>) {
        self.upload_options = options.clone();
    }

    fn set_headers(&mut self, headers: &Vec<&str>) {
        self.upload_headers = headers.iter().map(|item| (*item).to_owned()).collect();
    }

    fn set_upload_form_fields(&mut self, fields: &HashMap<&str, &str>) {
        let mut result = HashMap::new();
        for (key, value) in fields {
            result.insert(key.to_string(), value.to_string());
        }
        self.upload_form_fields = result;
    }
}

pub struct Manager {
    download_manager: Arc<Mutex<DownloadManager>>,
    upload_manager: Arc<Mutex<UploadManager>>,
}

impl Manager {
    pub fn new(
        download_url: &str,
        upload_url: &str,
        upload_options: Option<Vec<ManagerUploadOption>>,
        upload_headers: Option<Vec<String>>,
    ) -> Self {
        let upload_options = upload_options.unwrap_or(vec![]);
        let upload_headers = upload_headers.unwrap_or(vec![]);
        let http_boundary = String::from("---------------------------15875380808008");

        let (message_sender, message_receiver) = mpsc::channel::<ManagerThreadMessage>();
        let message_sender = Arc::new(Mutex::new(message_sender));
        let message_receiver = Arc::new(Mutex::new(message_receiver));

        let download_manager = Arc::new(Mutex::new(DownloadManager {
            download_url: download_url.to_owned(),
            download_headers: vec![],
            message_sender,
        }));

        let upload_manager = Arc::new(Mutex::new(UploadManager {
            download_url: download_url.to_owned(),
            upload_url: upload_url.to_owned(),
            upload_options,
            upload_form_fields: Default::default(),
            upload_headers,
            http_boundary,
            message_receiver,
        }));

        Self {
            download_manager,
            upload_manager,
        }
    }

    pub fn perform(&self) {
        let upload_thread = self.upload_manager.lock().unwrap().perform();

        self.download_manager.lock().unwrap().perform().unwrap();
        upload_thread.join().unwrap();

        println!("[i] Done");
    }

    pub async fn perform_async(&self) {
        self.perform();
    }

    pub fn set_upload_options(&self, options: &Vec<ManagerUploadOption>) {
        self.upload_manager
            .lock()
            .unwrap()
            .set_upload_options(options);
    }

    pub fn set_upload_headers(&self, headers: &Vec<&str>) {
        self.upload_manager.lock().unwrap().set_headers(headers);
    }

    pub fn set_upload_form_fields(&self, fields: &HashMap<&str, &str>) {
        self.upload_manager
            .lock()
            .unwrap()
            .set_upload_form_fields(fields);
    }

    pub fn set_download_headers(&self, headers: &Vec<&str>) {
        self.download_manager.lock().unwrap().set_headers(headers);
    }
}

fn vec_to_easy_list(list: &Vec<String>) -> List {
    let mut easy_list = List::new();

    let _: () = list
        .iter()
        .map(|item| easy_list.append(&item).unwrap())
        .collect();

    easy_list
}

fn get_header_string_value(data: String) -> Option<(String, String)> {
    let split_data: Vec<&str> = data.split(": ").collect();

    if split_data.len() > 1 {
        let key = (*split_data.get(0).unwrap()).to_owned();
        let other_values: Vec<&str> = split_data.iter().skip(1).map(|item| *item).collect();

        Some((key, other_values.join(": ")))
    } else {
        None
    }
}

fn guess_file_name_from_url(url: &String) -> String {
    let split_data: Vec<&str> = url.split("/").collect();
    let last_part = *split_data.iter().last().unwrap();

    let split_data: Vec<&str> = last_part.split("?").collect();
    (*split_data.get(0).unwrap()).to_owned()
}
