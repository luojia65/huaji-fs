use lazy_static::*;
use pest_derive::Parser;
use pest::Parser;
use std::collections::HashMap;
use std::io::{Result, ErrorKind, Read, Write};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::path::{Path, PathBuf, Component::*};

lazy_static! {
    static ref FILES: Arc<Mutex<HashMap<PathBuf, Arc<RwLock<FileSlot>>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
}

#[derive(Debug)]
pub struct File {
    pos: usize,
    slot: Weak<RwLock<FileSlot>>
}

#[derive(Debug, Eq, PartialEq, Hash)]
enum FileSlot {
    RandomAccess(Vec<u8>),
    // SymbolicLink(PathBuf),
    Directory,
}

impl File {
    fn from_slot_ref(slot: &Arc<RwLock<FileSlot>>) -> Self {
        File { pos: 0, slot: Arc::downgrade(slot) }
    }
}

impl File {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<File> {
        if let Some(slot) = FILES.lock().unwrap().get(path.as_ref()) {
            Ok(File::from_slot_ref(&slot))
        } else {
            Err(ErrorKind::NotFound.into())
        }
    }

    pub fn create<P: AsRef<Path>>(path: P) -> Result<File> {
        let new_file = Arc::new(RwLock::new(FileSlot::RandomAccess(Vec::new())));
        let ans = File::from_slot_ref(&new_file);
        FILES.lock().unwrap().insert(PathBuf::from(path.as_ref()), new_file);
        Ok(ans)
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let slot = if let Some(slot) = self.slot.upgrade() { slot } 
            else { return Err(ErrorKind::NotFound.into())};
        let mut slot = slot.write().unwrap();
        let vec = if let FileSlot::RandomAccess(vec) = &mut *slot { vec }
            else { return Err(ErrorKind::PermissionDenied.into())};
        vec.truncate(self.pos);
        vec.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let slot = if let Some(slot) = self.slot.upgrade() { slot } 
            else { return Err(ErrorKind::NotFound.into())};
        let mut slot = slot.write().unwrap();
        let vec = if let FileSlot::RandomAccess(vec) = &mut *slot { vec }
            else { return Err(ErrorKind::PermissionDenied.into())};
        let len = usize::max(buf.len(), vec.len() - self.pos);
        buf.copy_from_slice(&vec[self.pos..(self.pos + len)]);
        Ok(len)
    }
}

pub fn create_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    let new_file = Arc::new(RwLock::new(FileSlot::Directory));
    FILES.lock().unwrap().insert(PathBuf::from(path.as_ref()), new_file);
    Ok(())
}

#[derive(Parser)]
#[grammar = "command.pest"]
struct CommandParser;

fn append_path<P: AsRef<Path>>(cur_path: &mut PathBuf, delta: P) {
    for component in delta.as_ref().components() {
        match component {
            Prefix(_) => {}, 
            RootDir => *cur_path = PathBuf::from("/"),
            CurDir => {},
            ParentDir => { cur_path.pop(); },
            Normal(os_str) => cur_path.push(os_str),
        }
    }
}

fn contains_dir<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    if path.components().collect::<Vec<_>>() == vec![RootDir] {
        return true;
    }
    if let Some(slot) = FILES.lock().unwrap().get(&PathBuf::from(path)) {
        if let FileSlot::Directory = &*slot.read().unwrap() {
            return true;
        }
    }
    false
}

fn path_to_string<P: AsRef<Path>>(path: P) -> String {
    let mut ans = String::new();
    for component in path.as_ref().components() {
        match component {
            Prefix(_) => {}, 
            RootDir => {},
            CurDir => unreachable!(),
            ParentDir => unreachable!(),
            Normal(os_str) => ans += &os_str.to_string_lossy(),
        }
        ans.push('/')
    }
    ans

}

fn main() {
    let mut cur_path = PathBuf::from("/");
    loop {
        print!("[huaji {}] ", path_to_string(&cur_path));
        std::io::stdout().flush().unwrap();
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).unwrap();
        match CommandParser::parse(Rule::command, &buf.trim()) {
            Ok(mut pairs) => match pairs.next().map(|p| p.as_rule()) {
                Some(Rule::cmd_touch_head) => {
                    let file_name = pairs.next().map(|p| p.as_str()).unwrap();
                    let mut path = cur_path.clone();
                    append_path(&mut path, file_name);
                    File::create(path).unwrap();
                },
                Some(Rule::cmd_mkdir_head) => {
                    let file_name = pairs.next().map(|p| p.as_str()).unwrap();
                    let mut path = cur_path.clone();
                    append_path(&mut path, file_name);
                    create_dir(path).unwrap();
                },
                Some(Rule::cmd_ls_head) => {
                    for (path, file) in &*FILES.lock().unwrap() {
                        if path.parent() == Some(cur_path.as_path()) {
                            match &*file.read().unwrap() {
                                FileSlot::RandomAccess(_) => print!("FILE\t"),
                                FileSlot::Directory => print!("DIR\t"),
                            }
                            if let Some(file_name) = path.file_name() {
                                println!("{}", file_name.to_string_lossy())
                            }
                        }
                    }
                },
                Some(Rule::cmd_cd_head) => {
                    let file_name = pairs.next().map(|p| p.as_str()).unwrap();
                    let mut path = cur_path.clone();
                    append_path(&mut path, file_name);
                    if contains_dir(&path) {
                        cur_path = path
                    } else {
                        println!("cd: No such directory")
                    }
                },
                Some(Rule::cmd_huaji_head) => {
                    println!("Huaji");
                    let manager = battery::Manager::new();
                    for battery in manager.iter() {
                        println!("Vendor: {}", battery.vendor().unwrap_or("unknown"));
                        println!("Percentage: {} %", battery.percentage());
                        println!("Capacity: {:.2} %", battery.capacity());
                    }
                },
                Some(Rule::cmd_exit_head) => {
                    println!("logout");
                    return;
                }
                _ => {}
            },
            Err(e) => eprintln!("Err:{}", e)
        }
    }
}
