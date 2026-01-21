use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_SRC: &str = "/home/jef/Pictures/theframe";
const DEFAULT_DST: &str = "/home/jef/Pictures/display";
const DEFAULT_MAX_FILES: usize = 1200;
const DEFAULT_MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB

#[derive(Debug, Clone)]
struct Args {
    src: PathBuf,
    dst: PathBuf,
    max_files: usize,
    max_bytes: u64,
    seed: u64,
}

#[derive(Debug, Clone)]
struct FileInfo {
    path: PathBuf,
    name: String,
    size: u64,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args(env::args().collect())?;
    validate_dirs(&args)?;

    let mut files = collect_jpgs(&args.src)?;
    if files.is_empty() {
        return Err(format!(
            "no .jpg files found in source folder: {}",
            args.src.display()
        ));
    }

    shuffle_in_place(&mut files, args.seed);
    let groups = plan_groups(&files, args.max_files, args.max_bytes)?;

    copy_groups(&groups, &args.dst)?;
    print_summary(&groups, &args.dst);
    Ok(())
}

fn parse_args(argv: Vec<String>) -> Result<Args, String> {
    let mut src = PathBuf::from(DEFAULT_SRC);
    let mut dst = PathBuf::from(DEFAULT_DST);
    let mut max_files = DEFAULT_MAX_FILES;
    let mut max_bytes = DEFAULT_MAX_BYTES;
    let mut seed = default_seed();

    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--src" => {
                i += 1;
                src = PathBuf::from(required_arg(&argv, i, "--src")?);
            }
            "--dst" => {
                i += 1;
                dst = PathBuf::from(required_arg(&argv, i, "--dst")?);
            }
            "--max-files" => {
                i += 1;
                max_files = required_arg(&argv, i, "--max-files")?
                    .parse::<usize>()
                    .map_err(|_| "--max-files must be an integer".to_string())?;
                if max_files == 0 {
                    return Err("--max-files must be > 0".to_string());
                }
            }
            "--max-bytes" => {
                i += 1;
                max_bytes = required_arg(&argv, i, "--max-bytes")?
                    .parse::<u64>()
                    .map_err(|_| "--max-bytes must be an integer".to_string())?;
                if max_bytes == 0 {
                    return Err("--max-bytes must be > 0".to_string());
                }
            }
            "--seed" => {
                i += 1;
                seed = required_arg(&argv, i, "--seed")?
                    .parse::<u64>()
                    .map_err(|_| "--seed must be an integer".to_string())?;
            }
            other => {
                return Err(format!(
                    "unknown argument: {other}\n\nRun with --help for usage."
                ));
            }
        }
        i += 1;
    }

    Ok(Args {
        src,
        dst,
        max_files,
        max_bytes,
        seed,
    })
}

fn required_arg(argv: &[String], i: usize, flag: &str) -> Result<String, String> {
    argv.get(i)
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn print_help() {
    println!(
        "image-rando\n\n");
    println!(
        "Copies JPGs from a source folder into numbered destination folders (1..X),\n\
obeying:\n\
  - no more than 1200 photos per folder\n\
  - no more than 4 GiB per folder\n\n\
Default source: {DEFAULT_SRC}\n\
Default dest:   {DEFAULT_DST}\n\n\
USAGE:\n\
  cargo run --release -- [--src PATH] [--dst PATH] [--max-files N] [--max-bytes BYTES] [--seed SEED]\n"
    );
}

fn default_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    nanos ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

fn validate_dirs(args: &Args) -> Result<(), String> {
    let src_meta = fs::metadata(&args.src)
        .map_err(|e| format!("cannot read source folder {}: {e}", args.src.display()))?;
    if !src_meta.is_dir() {
        return Err(format!("source is not a directory: {}", args.src.display()));
    }

    fs::create_dir_all(&args.dst)
        .map_err(|e| format!("cannot create destination folder {}: {e}", args.dst.display()))?;
    let mut rd = fs::read_dir(&args.dst)
        .map_err(|e| format!("cannot read destination folder {}: {e}", args.dst.display()))?;
    if rd.next().is_some() {
        return Err(format!(
            "destination folder is not empty: {}\nRefusing to run to avoid mixing old/new output.",
            args.dst.display()
        ));
    }
    Ok(())
}

fn collect_jpgs(src: &Path) -> Result<Vec<FileInfo>, String> {
    let mut out = Vec::new();
    let rd = fs::read_dir(src)
        .map_err(|e| format!("cannot list source folder {}: {e}", src.display()))?;

    for entry in rd {
        let entry = entry.map_err(|e| format!("error reading directory entry: {e}"))?;
        let path = entry.path();
        let ft = entry
            .file_type()
            .map_err(|e| format!("cannot read file type for {}: {e}", path.display()))?;
        if !ft.is_file() {
            continue;
        }
        if !is_jpg(&path) {
            continue;
        }
        let meta = fs::metadata(&path)
            .map_err(|e| format!("cannot stat file {}: {e}", path.display()))?;
        let size = meta.len();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("non-utf8 filename not supported: {}", path.display()))?
            .to_string();

        out.push(FileInfo { path, name, size });
    }
    Ok(out)
}

fn is_jpg(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg"),
        None => false,
    }
}

// Simple, dependency-free RNG (xorshift64*) for shuffling.
#[derive(Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        // Avoid a zero state.
        let seed = if seed == 0 { 0xA5A5_A5A5_5A5A_5A5A } else { seed };
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

fn shuffle_in_place(files: &mut [FileInfo], seed: u64) {
    let mut rng = XorShift64::new(seed);
    // Fisher-Yates
    for i in (1..files.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        files.swap(i, j);
    }
}

fn plan_groups(files: &[FileInfo], max_files: usize, max_bytes: u64) -> Result<Vec<Vec<FileInfo>>, String> {
    let mut groups: Vec<Vec<FileInfo>> = Vec::new();
    let mut cur: Vec<FileInfo> = Vec::new();
    let mut cur_bytes: u64 = 0;

    for f in files {
        if f.size > max_bytes {
            return Err(format!(
                "file is larger than max-bytes ({} > {}): {}",
                f.size,
                max_bytes,
                f.path.display()
            ));
        }

        let would_exceed_files = !cur.is_empty() && (cur.len() + 1) > max_files;
        let would_exceed_bytes = !cur.is_empty() && (cur_bytes + f.size) > max_bytes;
        if would_exceed_files || would_exceed_bytes {
            groups.push(cur);
            cur = Vec::new();
            cur_bytes = 0;
        }

        cur_bytes += f.size;
        cur.push(f.clone());
    }

    if !cur.is_empty() {
        groups.push(cur);
    }
    Ok(groups)
}

fn copy_groups(groups: &[Vec<FileInfo>], dst_root: &Path) -> Result<(), String> {
    for (idx, group) in groups.iter().enumerate() {
        let folder_num = idx + 1;
        let folder = dst_root.join(folder_num.to_string());
        fs::create_dir_all(&folder)
            .map_err(|e| format!("cannot create folder {}: {e}", folder.display()))?;

        for f in group {
            let dest = folder.join(&f.name);
            if dest.exists() {
                return Err(format!(
                    "unexpected destination file already exists: {}",
                    dest.display()
                ));
            }
            fs::copy(&f.path, &dest)
                .map_err(|e| format!("failed to copy {} -> {}: {e}", f.path.display(), dest.display()))?;
        }
    }
    Ok(())
}

fn print_summary(groups: &[Vec<FileInfo>], dst_root: &Path) {
    let total_files: usize = groups.iter().map(|g| g.len()).sum();
    let total_bytes: u64 = groups
        .iter()
        .flat_map(|g| g.iter())
        .map(|f| f.size)
        .sum();

    println!("Copied {total_files} photos into {} folders under {}", groups.len(), dst_root.display());
    println!("Total bytes copied: {total_bytes}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fi(name: &str, size: u64) -> FileInfo {
        FileInfo {
            path: PathBuf::from(name),
            name: name.to_string(),
            size,
        }
    }

    #[test]
    fn plan_groups_respects_max_files() {
        let files = vec![fi("a.jpg", 1), fi("b.jpg", 1), fi("c.jpg", 1)];
        let groups = plan_groups(&files, 2, 10).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1].len(), 1);
    }

    #[test]
    fn plan_groups_respects_max_bytes() {
        let files = vec![fi("a.jpg", 6), fi("b.jpg", 6)];
        let groups = plan_groups(&files, 1200, 10).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0][0].name, "a.jpg");
        assert_eq!(groups[1][0].name, "b.jpg");
    }

    #[test]
    fn plan_groups_combines_until_limit() {
        let files = vec![fi("a.jpg", 6), fi("b.jpg", 4), fi("c.jpg", 1)];
        let groups = plan_groups(&files, 1200, 10).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2); // 6 + 4 = 10
        assert_eq!(groups[1].len(), 1);
    }

    #[test]
    fn plan_groups_errors_if_single_file_too_large() {
        let files = vec![fi("big.jpg", 11)];
        let err = plan_groups(&files, 1200, 10).unwrap_err();
        assert!(err.contains("larger than max-bytes"));
    }
}
