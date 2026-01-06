use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let build_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut file = File::create(out_dir.join("build_time.txt")).unwrap();
    writeln!(file, "{}", build_time).unwrap();
}
