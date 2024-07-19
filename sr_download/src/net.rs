// use Request::blocking::Client;

pub struct Downloader {
    pub client: Client,
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

fn list_file_in_dir(dir: &str, file_vec: &mut Vec<u64>) -> Result<(), Box<dyn std::error::Error>> {
    let save_paths = std::fs::read_dir(dir)?;
    for save_path in save_paths {
        if let Ok(save_path) = save_path {
            let file_name = save_path.file_name();
            let file_name = file_name.to_str().unwrap();
            // 判断文件名是否以 .xml 结尾
            if file_name.ends_with(".xml") {
                let file_id = file_name.trim_end_matches(".xml");
                let file_id = file_id.parse::<u64>()?;
                file_vec.push(file_id);
            }
        }
    }
    file_vec.sort();
    Ok(())
}

enum FileKind {
    Ship,
    Save,
}

//  飞船 API http://jundroo.com/service/SimpleRockets/DownloadRocket?id=
//  存档 API http://jundroo.com/service/SimpleRockets/DownloadSandBox?id=
//  curl http://jundroo.com/service/SimpleRockets/DownloadRocket?id=144444
// fn get_file_from_jundroo(id: u64) -> Result<FileKind, Box<dyn std::error::Error>> {
//     let ship_try = reqwest::blocking::get(format!(
//         "http://jundroo.com/service/SimpleRockets/DownloadRocket?id={}",
//         id
//     ))?;
//     println!("ship_try: {:?}", ship_try);
//     println!("body: {}", ship_try.text()?);

//     Ok(FileKind::Ship)
// }
