use Request::blocking::Client;

pub struct Downloader {
    pub client: Client
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            client: Client::new()
        }
    }
}
