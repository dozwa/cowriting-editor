//! What the core needs from where it runs: files, the clock, ids. The app gives it the disk;
//! tests give it memory.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Host: Send + Sync {
    fn read(&self, path: &str) -> io::Result<String>;
    /// Writes the whole file, creating its folders.
    fn write(&self, path: &str, text: &str) -> io::Result<()>;
    /// Adds to the end of the file, creating it and its folders.
    fn append(&self, path: &str, text: &str) -> io::Result<()>;
    fn remove(&self, path: &str) -> io::Result<()>;
    fn exists(&self, path: &str) -> bool;
    /// The names of the files directly in a folder.
    fn list(&self, path: &str) -> io::Result<Vec<String>>;
    /// The names of the folders directly in a folder.
    fn folders(&self, path: &str) -> io::Result<Vec<String>>;
    /// Moves a file, or a folder with everything in it, creating the folders above its new place.
    fn rename(&self, from: &str, to: &str) -> io::Result<()>;
    /// Now, as ISO 8601 in UTC with milliseconds: `2026-11-23T10:00:00.000Z`.
    fn now(&self) -> String;
    /// A new log id: 12 hex characters.
    fn new_id(&self) -> String;
}

/// A moment as ISO 8601 in UTC with milliseconds.
pub fn iso_utc(at: SystemTime) -> String {
    let since = at.duration_since(UNIX_EPOCH).unwrap_or_default();
    let (seconds, millis) = (since.as_secs() as i64, since.subsec_millis());
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The real disk and the real clock.
#[derive(Default)]
pub struct DiskHost;

impl Host for DiskHost {
    fn read(&self, path: &str) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &str, text: &str) -> io::Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)
    }

    fn append(&self, path: &str, text: &str) -> io::Result<()> {
        use std::io::Write;
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(text.as_bytes())
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn list(&self, path: &str) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        Ok(names)
    }

    fn folders(&self, path: &str) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        Ok(names)
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if let Some(parent) = Path::new(to).parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::fs::rename(from, to).is_ok() {
            return Ok(());
        }
        // another drive: copy everything, then remove the original
        fn copy(from: &Path, to: &Path) -> io::Result<()> {
            if from.is_dir() {
                std::fs::create_dir_all(to)?;
                for entry in std::fs::read_dir(from)? {
                    let entry = entry?;
                    copy(&entry.path(), &to.join(entry.file_name()))?;
                }
                Ok(())
            } else {
                std::fs::copy(from, to).map(|_| ())
            }
        }
        copy(Path::new(from), Path::new(to))?;
        if Path::new(from).is_dir() {
            std::fs::remove_dir_all(from)
        } else {
            std::fs::remove_file(from)
        }
    }

    fn now(&self) -> String {
        iso_utc(SystemTime::now())
    }

    fn new_id(&self) -> String {
        let mut bytes = [0u8; 6];
        getrandom::getrandom(&mut bytes).expect("the system has randomness");
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Files in memory, a clock that stands still until it is set, and ids that count up.
pub struct MemoryHost {
    files: Mutex<BTreeMap<String, String>>,
    now: Mutex<String>,
    ids: Mutex<u64>,
}

impl Default for MemoryHost {
    fn default() -> Self {
        MemoryHost { files: Mutex::default(), now: Mutex::new("2026-11-23T10:00:00.000Z".into()), ids: Mutex::new(0) }
    }
}

impl MemoryHost {
    pub fn set_now(&self, iso: &str) {
        *self.now.lock().unwrap() = iso.to_string();
    }

    /// Every file, by path.
    pub fn files(&self) -> BTreeMap<String, String> {
        self.files.lock().unwrap().clone()
    }
}

impl Host for MemoryHost {
    fn read(&self, path: &str) -> io::Result<String> {
        self.files.lock().unwrap().get(path).cloned().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.to_string()))
    }

    fn write(&self, path: &str, text: &str) -> io::Result<()> {
        self.files.lock().unwrap().insert(path.to_string(), text.to_string());
        Ok(())
    }

    fn append(&self, path: &str, text: &str) -> io::Result<()> {
        self.files.lock().unwrap().entry(path.to_string()).or_default().push_str(text);
        Ok(())
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.files.lock().unwrap().remove(path);
        Ok(())
    }

    fn exists(&self, path: &str) -> bool {
        let files = self.files.lock().unwrap();
        let folder = format!("{}/", path.trim_end_matches('/'));
        files.contains_key(path) || files.keys().any(|k| k.starts_with(&folder))
    }

    fn list(&self, path: &str) -> io::Result<Vec<String>> {
        let folder = format!("{}/", path.trim_end_matches('/'));
        Ok(self.files.lock().unwrap().keys().filter_map(|k| k.strip_prefix(&folder)).filter(|rest| !rest.contains('/')).map(String::from).collect())
    }

    fn folders(&self, path: &str) -> io::Result<Vec<String>> {
        let folder = format!("{}/", path.trim_end_matches('/'));
        let files = self.files.lock().unwrap();
        let mut names: Vec<String> = files.keys().filter_map(|k| k.strip_prefix(&folder)).filter_map(|rest| rest.split_once('/')).map(|(name, _)| name.to_string()).collect();
        names.dedup();
        Ok(names)
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let mut files = self.files.lock().unwrap();
        let below = format!("{from}/");
        let moved: Vec<String> = files.keys().filter(|k| k.as_str() == from || k.starts_with(&below)).cloned().collect();
        if moved.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, from.to_string()));
        }
        for old in moved {
            let text = files.remove(&old).expect("just listed");
            files.insert(format!("{to}{}", &old[from.len()..]), text);
        }
        Ok(())
    }

    fn now(&self) -> String {
        self.now.lock().unwrap().clone()
    }

    fn new_id(&self) -> String {
        let mut ids = self.ids.lock().unwrap();
        *ids += 1;
        format!("{:012x}", *ids)
    }
}
