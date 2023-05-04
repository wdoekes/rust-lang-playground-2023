use std::{
    env,
    error,
    //io,
    fs,
    path,
    //str,
};

use inotify::{
    EventMask,
    Inotify,
    WatchMask,
};

// If we return "Result<(), io::Error>" we cannot handle other errors.
// If we instead return "Result<(), Box<dyn error::Error>>" we can
// return a multitude of errors automatically. Now both
// "metadata.modified()?" and "...elapsed()?" can have the
// error-propagating-question mark.
fn do_traverse(root_dir: &path::PathBuf) -> Result<(), Box<dyn error::Error>> {
    println!("Entries modified in the last 24 hours in {:?}:", root_dir);

    for entry in fs::read_dir(root_dir)? {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        let last_modified = metadata.modified()?.elapsed()?.as_secs();

        if last_modified < 24 * 3600 && metadata.is_file() {
            println!(
                "Last modified: {:?} seconds, is read only: {:?}, size: {:?} bytes, filename: {:?}",
                last_modified,
                metadata.permissions().readonly(),
                metadata.len(),
                path.file_name().ok_or("No filename")
            );
        }
    }
    Ok(())
}

fn do_some_error() -> Result<(), Box<dyn error::Error>> {
    //str::from_utf8(b"\x81")?; // INVALID!
    //return Err(Box::new(io::Error::new(io::ErrorKind::Other, "Something went wrong")));
    Ok(())
}

fn do_inotify(root_dir: &path::PathBuf) -> Result<(), Box<dyn error::Error>> {
    let mut inotify = Inotify::init()?;

    inotify
        .add_watch(
            root_dir,
            WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
        )
        .expect("Failed to add inotify watch");

    println!("Watching current directory for activity...");

    let mut buffer = [0u8; 4096];
    loop {
        let events = inotify
            .read_events_blocking(&mut buffer)
            .expect("Failed to read inotify events");

        for event in events {
            if event.mask.contains(EventMask::CREATE) {
                if event.mask.contains(EventMask::ISDIR) {
                    println!("Directory created: {:?}", event.name);
                } else {
                    println!("File created: {:?}", event.name);
                }
            } else if event.mask.contains(EventMask::DELETE) {
                if event.mask.contains(EventMask::ISDIR) {
                    println!("Directory deleted: {:?}", event.name);
                } else {
                    println!("File deleted: {:?}", event.name);
                }
            } else if event.mask.contains(EventMask::MODIFY) {
                if event.mask.contains(EventMask::ISDIR) {
                    println!("Directory modified: {:?}", event.name);
                } else {
                    println!("File modified: {:?}", event.name);
                }
            }
        }
    }
    //Ok(()) // unreachable for now
}

fn run() -> Result<(), Box<dyn error::Error>> {
    let args: Vec<String> = env::args().collect();

    let root_dir: path::PathBuf;
    match args.len() {
        1 => { root_dir = env::current_dir().expect("No current dir"); },
        _ => { root_dir = path::PathBuf::from(&args[1]); },
    }
    do_some_error()?;
    do_traverse(&root_dir)?;
    do_inotify(&root_dir)?;
    Ok(())
}

fn main() {
    match run() {
        Ok(_) => {},
        Err(msg) => { eprintln!("Error: {msg}"); std::process::exit(1); },
    }
}
