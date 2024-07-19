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

/// 飞船 API http://jundroo.com/service/SimpleRockets/DownloadRocket?id=
/// 存档 API http://jundroo.com/service/SimpleRockets/DownloadSandBox?id=
/// curl http://jundroo.com/service/SimpleRockets/DownloadRocket?id=144444
fn get_file_from_jundroo(id: u64) -> Result<FileKind, Box<dyn std::error::Error>> {
    let ship_try = reqwest::blocking::get(format!(
        "http://jundroo.com/service/SimpleRockets/DownloadRocket?id={}",
        id
    ))?;
    println!("ship_try: {:?}", ship_try);
    println!("body: {}", ship_try.text()?);

    Ok(FileKind::Ship)
}

fn main() {
    let mut ship_vec: Vec<u64> = Vec::with_capacity(82_0000);
    let mut save_vec: Vec<u64> = Vec::with_capacity(29_0000);

    list_file_in_dir("./ship", &mut ship_vec).unwrap();
    list_file_in_dir("./save", &mut save_vec).unwrap();

    let _ = get_file_from_jundroo(144444);
    for i in 144444..144450 {
        let _ = get_file_from_jundroo(i);
    }

    println!("ship: {}, save: {}", ship_vec.len(), save_vec.len());
}
