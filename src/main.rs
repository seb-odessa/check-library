use md5;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, Read, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
struct Md5Cache {
    md5map: HashMap<String, String>,
}

impl Md5Cache {
    fn load(cache_file: &Path) -> Result<Self, io::Error> {
        if cache_file.exists() {
            let file = File::open(cache_file)?;
            let reader = io::BufReader::new(file);
            let cache: Md5Cache = serde_json::from_reader(reader)?;
            Ok(cache)
        } else {
            Ok(Md5Cache {
                md5map: HashMap::new(),
            })
        }
    }

    fn save(&self, cache_file: &Path) -> Result<(), io::Error> {
        let file = File::create(cache_file)?;
        let writer = io::BufWriter::new(file);
        serde_json::to_writer(writer, &self)?;
        Ok(())
    }
}

fn calculate_md5(file_path: &Path) -> Result<String, io::Error> {
    let mut file = File::open(file_path)?;
    let mut hasher = md5::Context::new();
    let mut buffer = [0; 1024 * 1024]; // 1 Мб
    let mut count = 1;
    let step = 100;
    let spaces = ' '.to_string().repeat(step);
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.consume(&buffer[..bytes_read]);
        print!(".");
        if 0 == count % step {
            write!(io::stdout(), "\x1b[{step}D")?;
            print!("{spaces}");
            write!(io::stdout(), "\x1b[{step}D")?;
        }
        io::stdout().flush()?;
        count += 1;

    }

    let result = hasher.compute();
    Ok(format!("{:x}", result))
}

fn load_md5map(md5sum_path: &Path) -> Result<HashMap<String, String>, io::Error> {
    let file = File::open(md5sum_path)?;
    let reader = io::BufReader::new(file);
    let mut md5map = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 {
            let md5_sum = parts[0].to_string();
            let file_path = parts[1].replace('*', "/").replace('\\', "/");
            md5map.insert(file_path, md5_sum);
        }
    }
    Ok(md5map)
}

fn check_integrity(
    root: &Path,
    md5sum: &Path,
    cache_file: &Path,
    regex: &Regex,
) -> Result<(), io::Error> {
    let mut cache = Md5Cache::load(cache_file)?;
    let md5map = load_md5map(md5sum)?;

    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_file() {
            let relative_path = path
                .strip_prefix(root)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .to_str()
                .unwrap()
                .replace('\\', "/");

            if regex.is_match(&relative_path) {
                let full_path = path.display().to_string();
                let cached_md5 = cache.md5map.get(&full_path).cloned();

                if let Some(expected_md5) = md5map.get(&full_path) {
                    print!("{full_path}.");
                    io::stdout().flush()?;

                    let calculated_md5 = match cached_md5 {
                        Some(cached_md5) => cached_md5,
                        None => {
                            let md5 = calculate_md5(&path)?;
                            cache.md5map.insert(full_path.clone(), md5.clone());
                            cache.save(cache_file)?;
                            md5
                        }
                    };

                    if &calculated_md5 != expected_md5 {
                        println!(".FAIL. Expected: {expected_md5}, Actual: {calculated_md5}");
                    } else {
                        println!(".OK.");
                    }
                } else {
                    println!("{full_path} absent in {}.", md5sum.display());
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), io::Error> {
    let root = Path::new("/lib.rus.ec");
    let md5sum = Path::new("/lib.rus.ec/librusec_local.md5");
    let cache = Path::new("/lib.rus.ec/md5_cache.json");
    // let regex = Regex::new(r"^fb2-\d+-\d+(_lost)?\.zip$").unwrap();

    let regex = Regex::new(r"^fb2-0\d+-\d+(_lost)?\.zip$").unwrap();

    check_integrity(root, md5sum, cache, &regex)?;
    Ok(())
}
