use curl::easy::{Easy, List};
use std::{
    io::Read,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

enum ManagerThreadMessage {
    Bytes(Box<[u8]>),
    StopSignal,
}

#[derive(Clone, Copy)]
pub enum ManagerUploadRequestType {
    POST,
    PUT,
}

pub enum ManagerUploadOption {
    FieldName(String),
    FileName(String),
    ContentType(String),
    RequestType(ManagerUploadRequestType),
}

#[allow(dead_code)]
struct ManagerCommunicator {
    sender: Arc<Mutex<Sender<ManagerThreadMessage>>>,
    extra_sender: Arc<Mutex<Sender<ManagerThreadMessage>>>,
    receiver: Arc<Mutex<Receiver<ManagerThreadMessage>>>,
}

pub struct Manager {
    download_url: String,
    download_request: Arc<Mutex<Easy>>,
    download_headers: List,
    upload_url: String,
    upload_request: Arc<Mutex<Easy>>,
    upload_options: Vec<ManagerUploadOption>,
    upload_headers: List,
    upload_http_boundary: String,
    channel: ManagerCommunicator,
}

impl Manager {
    pub fn new(
        download_url: &str,
        upload_url: &str,
        upload_options: Option<Vec<ManagerUploadOption>>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<ManagerThreadMessage>();
        let sender = Arc::new(Mutex::new(sender));
        let receiver = Arc::new(Mutex::new(receiver));
        let extra_sender = sender.clone();

        let mut final_upload_options = vec![];
        if let Some(options) = upload_options {
            final_upload_options = options;
        }

        let mut final_request_type = ManagerUploadRequestType::POST;
        let _: () = final_upload_options
            .iter()
            .map(|item| {
                if let ManagerUploadOption::RequestType(request_type) = item {
                    final_request_type = request_type.clone();
                }
            })
            .collect();

        let download_headers = List::new();
        let upload_http_boundary = String::from("---------------------------15875380808008");
        let mut upload_headers = List::new();
        upload_headers
            .append(&format!(
                "Content-Type: multipart/form-data; boundary={upload_http_boundary}"
            ))
            .unwrap();

        let download_request = Arc::new(Mutex::new(Self::create_download_request(
            download_url.to_owned(),
            Arc::clone(&sender),
        )));
        let upload_request = Arc::new(Mutex::new(Self::create_upload_request(
            upload_url.to_owned(),
            Arc::clone(&receiver),
            final_request_type,
        )));

        let download_url = download_url.to_owned();
        let upload_url = upload_url.to_owned();

        Self {
            download_url,
            download_request,
            download_headers,
            upload_url,
            upload_request,
            upload_headers,
            upload_options: final_upload_options,
            upload_http_boundary,
            channel: ManagerCommunicator {
                sender,
                extra_sender,
                receiver,
            },
        }
    }

    pub fn perform(&self) {
        self.apply_upload_headers();
        let post_request = Arc::clone(&self.upload_request);
        let post_thread = thread::spawn(move || {
            post_request.lock().unwrap().perform().unwrap();
        });

        self.send_initial_data();

        println!("[i] Starting to receive and send data, server response will be printed");

        self.apply_download_headers();
        self.download_request.lock().unwrap().perform().unwrap();

        self.send_final_data();

        post_thread.join().unwrap();

        println!("[i] Done");
    }

    pub async fn perform_async(&self) {
        self.perform();
    }

    fn create_download_request(
        download_url: String,
        data_sender: Arc<Mutex<Sender<ManagerThreadMessage>>>,
    ) -> Easy {
        let download_url = download_url.to_owned();

        let mut get = Easy::new();
        get.url(&download_url).unwrap();
        let sender = data_sender;
        get.write_function(move |data| {
            let mut copied_data = vec![0; data.len()];
            copied_data.copy_from_slice(data);
            sender
                .lock()
                .unwrap()
                .send(ManagerThreadMessage::Bytes(copied_data.into_boxed_slice()))
                .unwrap();
            Ok(data.len())
        })
        .unwrap();

        get
    }

    fn create_upload_request(
        upload_url: String,
        data_receiver: Arc<Mutex<Receiver<ManagerThreadMessage>>>,
        request_type: ManagerUploadRequestType,
    ) -> Easy {
        let upload_url = upload_url.to_owned();

        let mut post = Easy::new();
        post.url(&upload_url).unwrap();
        match request_type {
            ManagerUploadRequestType::POST => post.post(true).unwrap(),
            ManagerUploadRequestType::PUT => post.put(true).unwrap(),
        }
        post.read_function(move |buf| {
            let message = data_receiver.lock().unwrap().recv().unwrap();
            if let ManagerThreadMessage::Bytes(data) = message {
                Ok(data.as_ref().read(buf).unwrap())
            } else if let ManagerThreadMessage::StopSignal = message {
                Ok(0)
            } else {
                panic!("Received non bytes message");
            }
        })
        .unwrap();
        post.write_function(|data| {
            let mut buf = String::new();
            data.clone().read_to_string(&mut buf).unwrap();
            println!("{}", buf);
            Ok(data.len())
        })
        .unwrap();

        post
    }

    fn apply_download_headers(&self) {
        self.download_request
            .lock()
            .unwrap()
            .http_headers(clone_easy_list(&self.download_headers))
            .unwrap();
    }

    fn apply_upload_headers(&self) {
        self.upload_request
            .lock()
            .unwrap()
            .http_headers(clone_easy_list(&self.upload_headers))
            .unwrap();
    }

    fn send_initial_data(&self) {
        let mut fieldname = String::from("file");
        let mut filename = String::from("file");
        let mut content_type = String::from("");
        let upload_http_boundary = self.upload_http_boundary.to_owned();
        let _: () = self
            .upload_options
            .iter()
            .map(|item| match item {
                ManagerUploadOption::FieldName(value) => fieldname = value.to_owned(),
                ManagerUploadOption::FileName(value) => filename = value.to_owned(),
                ManagerUploadOption::ContentType(value) => content_type = value.to_owned(),
                ManagerUploadOption::RequestType(_) => (),
            })
            .collect();

        let mut content_type_line = String::from("");
        if content_type.len() > 0 {
            content_type_line = format!("\nContent-Type: {content_type}")
        }

        self.channel.extra_sender
        .lock().unwrap()
        .send(ManagerThreadMessage::Bytes(
            format!("--{upload_http_boundary}\r\nContent-Disposition: form-data; name=\"{fieldname}\"; filename=\"{filename}\"{content_type_line}\r\n\r\n")
                .as_bytes()
                .to_vec()
                .into_boxed_slice(),
        ))
        .unwrap();
    }

    fn send_final_data(&self) {
        let upload_http_boundary = self.upload_http_boundary.to_owned();

        self.channel
            .extra_sender
            .lock()
            .unwrap()
            .send(ManagerThreadMessage::Bytes(
                format!("\r\n--{upload_http_boundary}--\r\n")
                    .as_bytes()
                    .to_vec()
                    .into_boxed_slice(),
            ))
            .unwrap();

        self.channel
            .extra_sender
            .lock()
            .unwrap()
            .send(ManagerThreadMessage::StopSignal)
            .unwrap();
    }

    pub fn download_url(&self) -> String {
        self.download_url.to_owned()
    }

    pub fn download_headers(&mut self) -> &mut List {
        &mut self.download_headers
    }

    pub fn upload_url(&self) -> String {
        self.upload_url.to_owned()
    }

    pub fn upload_options(&mut self) -> &mut Vec<ManagerUploadOption> {
        &mut self.upload_options
    }

    pub fn upload_headers(&mut self) -> &mut List {
        &mut self.upload_headers
    }
}

fn clone_easy_list(list: &List) -> List {
    let mut new_list = List::new();

    let _: () = list
        .iter()
        .map(|item| {
            let mut copied_value = String::new();
            item.clone().read_to_string(&mut copied_value).unwrap();
            new_list.append(&copied_value).unwrap();
        })
        .collect();

    new_list
}
