use md5;
use memmap2::Mmap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
struct Md5Cache {
    md5map: BTreeMap<String, String>,
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
                md5map: BTreeMap::new(),
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
    let file = File::open(file_path)?;
    let mut count = 1;
    let step = 30;
    let spaces = ' '.to_string().repeat(step);
    let window_size: usize = 10 * 1024 * 1024;

    let mmap = unsafe { Mmap::map(&file)? };

    let mut offset = 0;
    let mut hasher = md5::Context::new();
    while offset < mmap.len() {
        let end = std::cmp::min(offset + window_size, mmap.len());
        let window = &mmap[offset..end];
        hasher.consume(window);

        print!(".");
        if 0 == count % step {
            write!(io::stdout(), "\x1b[{step}D")?;
            print!("{spaces}");
            write!(io::stdout(), "\x1b[{step}D")?;
        }
        io::stdout().flush()?;
        count += 1;

        offset += window_size;
    }

    let result = hasher.compute();
    Ok(format!("{:x}", result))
}

fn load_md5map(
    root: &Path,
    md5sum_path: &Path,
    regex: &Regex,
) -> Result<BTreeMap<String, String>, io::Error> {
    let file = File::open(md5sum_path)?;
    let reader = io::BufReader::new(file);
    let mut md5map = BTreeMap::new();
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 {
            let md5_sum = parts[0].to_string();
            let file_path = parts[1].replace('*', "/").replace('\\', "/");

            if let Ok(file_name) = Path::new(&file_path).strip_prefix(root) {
                if let Some(file_name) = file_name.to_str() {
                    if regex.is_match(&file_name) {
                        md5map.insert(file_path, md5_sum);
                    } else {

                    }
                }
            } else {
                if file_path.ends_with("inpx") {
                    let inpx = root.display().to_string() + &file_path;
                    md5map.insert(inpx, md5_sum);
                }
            }
        }
    }
    Ok(md5map)
}

fn get_filtered_and_sorted(root: &Path, regex: &Regex) -> Result<Vec<String>, io::Error> {
    let mut files = Vec::new();

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
                files.push(path.display().to_string());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn find_missing_files(expected: &BTreeMap<String, String>, calculated: &BTreeMap<String, String>) {
    for key in expected.keys() {
        if !calculated.contains_key(key) {
            println!("{key} is missed or was not handled");
        }
    }
}

fn check_integrity(
    root: &Path,
    md5sum: &Path,
    cache_file: &Path,
    regex: &Regex,
) -> Result<(), io::Error> {
    let mut cache = Md5Cache::load(cache_file)?;
    let md5map = load_md5map(root, md5sum, regex)?;
    let filtered = get_filtered_and_sorted(root, regex)?;

    for file in filtered.into_iter() {
        let cached_md5 = cache.md5map.get(&file).cloned();

        if let Some(expected_md5) = md5map.get(&file) {
            print!("{file}.");
            io::stdout().flush()?;

            let calculated_md5 = match cached_md5 {
                Some(cached_md5) => cached_md5,
                None => {
                    let path = Path::new(&file);
                    let md5 = calculate_md5(&path)?;
                    cache.md5map.insert(file.clone(), md5.clone());
                    cache.save(cache_file)?;
                    md5
                }
            };

            if &calculated_md5 != expected_md5 {
                println!(".FAIL.");
                println!("Expected: {expected_md5}, Actual: {calculated_md5}");
                cache.md5map.remove(&file);
                cache.save(cache_file)?;
            } else {
                println!(".OK.");
            }
        } else {
            println!("{file} absent in {}.", md5sum.display());
        }
    }
    find_missing_files(&md5map, &cache.md5map);
    Ok(())
}

fn main() -> Result<(), io::Error> {
    let root = Path::new("/lib.rus.ec");
    let md5sum = Path::new("/lib.rus.ec/librusec_local.md5");
    let cache = Path::new("/lib.rus.ec/md5_cache.json");
    let regex = Regex::new(r"^fb2-\d+-\d+(_lost)?\.zip$|^librusec_local_fb2\.inpx$").unwrap();
    check_integrity(root, md5sum, cache, &regex)?;
    Ok(())
}
