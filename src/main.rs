use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::path::PathBuf;

/// Utility for splitting and merging files
#[derive(Parser, Debug)]
#[command(name = "esplitter")]
#[command(version = "0.0.1")]
#[command(about = "", long_about = None)]
struct Config {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Units {
    B,
    Kb,
    Mb,
    Gb,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Split file into parts
    Split {
        #[arg(short, long)]
        path: PathBuf,

        #[arg(short, long)]
        size: usize,

        #[arg(short, long)]
        units: Units,
    },

    /// Make one file from several parts
    Merge {
        #[arg(short, long)]
        path: PathBuf,
    },
}

/// Split big file into several small parts
///
/// This function splits a file into multiple smaller files, each with a maximum size
/// specified by `part_size`. The resulting files are named with the original filename
/// followed by `.part.NNN` where NNN is the part number (zero-padded to 3 digits).
///
/// # Arguments
///
/// * `path` - Path to the file to be split
/// * `part_size` - Maximum size in bytes for each part
///
/// # Returns
///
/// Returns `Ok(())` if the file was successfully split or if the file is already
/// smaller than or equal to `part_size`. Returns an error if:
/// - The file does not exist or is not a regular file
/// - `part_size` is 0
/// - The file is empty
/// - An I/O error occurs during reading or writing
///
/// # Examples
/// /// ## Basic usage
///
/// ```rust
/// # use std::fs;
/// # use std::io::Write;
/// # use std::path::PathBuf;
/// # fn main() -> std::io::Result<()> {
/// #     let temp_dir = tempfile::tempdir()?;
/// #     let file_path = temp_dir.path().join("test.txt");
/// #     let mut file = fs::File::create(&file_path)?;
/// #     file.write_all(&vec![0u8; 1000])?;
/// #     
/// // Split a 1000-byte file into 300-byte parts
/// split(&file_path, 300)?;
///
/// // Creates: test.txt.part.001 (300 bytes)
/// //          test.txt.part.002 (300 bytes)
/// //          test.txt.part.003 (300 bytes)
/// //          test.txt.part.004 (100 bytes)
/// #     assert!(temp_dir.path().join("test.txt.part.001").exists());
/// #     assert!(temp_dir.path().join("test.txt.part.002").exists());
/// #     assert!(temp_dir.path().join("test.txt.part.003").exists());
/// #     assert!(temp_dir.path().join("test.txt.part.004").exists());
/// #     Ok(())
/// # }
/// ```
/// # Notes
///
/// - The original file is not modified or deleted
/// - Part files are created in the same directory as the original file
/// - Part numbers start from 1 and are zero-padded to 3 digits (001, 002, etc.)
/// - The function uses a 64KB buffer for reading, which provides good performance
///   for most use cases
pub fn split(path: &PathBuf, part_size: usize) -> io::Result<()> {
    let meta = fs::metadata(path)?;
    if !path.exists() || !meta.is_file() || part_size == 0 || meta.len() == 0 {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }

    if meta.len() as usize <= part_size {
        return Ok(());
    }

    let from = fs::File::open(path)?;
    let mut reader = io::BufReader::new(from);
    let mut buffer = vec![0u8; 64 * 1024];
    let mut part_number = 1;
    let mut total_bytes = meta.len() as usize;

    let gen_part_name =
        move |part_number: i32| -> String { format!("{}.part.{:03}", path.display(), part_number) };

    while total_bytes > 0 {
        let readen = reader.read(&mut buffer)?;
        if readen == 0 {
            break;
        }

        let mut start_idx = 0;

        while start_idx < readen {
            let part_filename = gen_part_name(part_number);

            let mut to_file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part_filename)?;

            let current_size = to_file.metadata()?.len() as usize;
            let remaining_in_part = part_size.saturating_sub(current_size);

            if remaining_in_part == 0 {
                part_number += 1;
                continue;
            }

            let bytes_to_write = std::cmp::min(readen - start_idx, remaining_in_part);
            let written = to_file.write(&buffer[start_idx..start_idx + bytes_to_write])?;

            start_idx += written;

            if current_size + written >= part_size {
                part_number += 1;
            }
        }

        total_bytes -= readen;
    }

    Ok(())
}

/// Merge several parts of a file into one big file
///
/// The function searches the source file's directory for all files matching the pattern
/// `<filename>.part.<NNN>`, sorts them, and sequentially writes them into the target file.
///
/// # Arguments
///
/// * `path` - Path to the target (original) file, which will be created or overwritten
///
/// # Returns
///
/// Returns `Ok(())` upon successful merge. Returns an error if:
/// - No part files are found
/// - An I/O error occurs while reading parts or writing to the target file
pub fn merge(path: &PathBuf, is_clean: bool) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file path"))?;

    let prefix = format!("{}.part.", file_name);
    let mut part_files = Vec::new();

    // 1. Search for all file parts in the directory
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_str().unwrap_or("");

        if name_str.starts_with(&prefix) {
            let suffix = &name_str[prefix.len()..];
            // Check that the suffix consists of exactly 3 digits (protection against accidental matches)
            if suffix.len() == 3 && suffix.chars().all(|c| c.is_ascii_digit()) {
                part_files.push(entry.path());
            }
        }
    }

    if part_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No part files found for '{}'", file_name),
        ));
    }

    // 2. Sort parts (lexicographic sorting works thanks to zero-padding 001, 002...)
    part_files.sort();

    // 3. Create target file (overwrites existing or creates new)
    let mut output_file = fs::File::create(path)?;
    let mut buffer = vec![0u8; 64 * 1024]; // 64 KB buffer, same as in split

    // 4. Sequential reading and writing
    for part_path in &part_files {
        let mut part_file = fs::File::open(part_path)?;
        loop {
            let bytes_read = part_file.read(&mut buffer)?;
            if bytes_read == 0 {
                break; // End of file
            }
            output_file.write_all(&buffer[..bytes_read])?;
        }
    }

    // Delete parts after successful merge
    if is_clean {
        for part_path in &part_files {
            fs::remove_file(part_path)?;
        }
    }

    Ok(())
}

fn main() {
    let config = Config::parse();

    match &config.command {
        Commands::Split { path, size, units } => {
            let bytes = match units {
                Units::B => *size,
                Units::Kb => *size * 1024,
                Units::Mb => *size * 1024 * 1024,
                Units::Gb => *size * 1024 * 1024 * 1024,
            };

            split(path, bytes).expect("Failed to split file");
            println!("File successfully split.");
        }
        Commands::Merge { path } => {
            merge(path, true).expect("Failed to merge file");
            println!("File successfully merged.");
        }
    }
}

#[cfg(test)]
mod tests;
