use anyhow::{Context, Result, anyhow};
use memchr::memmem;
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::PathBuf,
};

pub struct SDFile {
    path: PathBuf,
    byte_offsets: Vec<u64>,
    byte_len: u64,
    file: Option<File>,
}

impl SDFile {
    /// Read the records from a path to an sdf file.
    pub fn open(path: PathBuf) -> Result<Self> {
        let mut file = File::open(&path)
            .with_context(|| format!("Failed to open SDF file: {}", path.display()))?;
        let byte_len = file
            .metadata()
            .with_context(|| format!("Failed to find metadat stat: {}", path.display()))?
            .len();
        let byte_offsets = scan_offsets(&mut file, byte_len)
            .with_context(|| format!("Failed to index records in: {}", path.display()))?;
        Ok(Self {
            path,
            byte_offsets,
            byte_len,
            file: Some(file),
        })
    }

    /// Number of records in the file.
    pub fn len(&self) -> usize {
        self.byte_offsets.len()
    }
    /// Check if a file is empty in the file.
    pub fn is_empty(&self) -> bool {
        self.byte_offsets.is_empty()
    }
}

/// Scan the records in a file and return the byte offsets for the start of each record.
pub fn scan_offsets(file: &mut File, total_file_bytes: u64) -> Result<Vec<u64>> {
    // Use a cache compatible buffer size
    const BUF_SIZE: usize = 1 << 17; // 128 KiB
    let delimiter = "$$$$".as_bytes();
    // memchr fast find for delimiter
    let finder = memmem::Finder::new(delimiter);

    file.seek(SeekFrom::Start(0))?;
    // Only load buffer sized amounts of the file at once
    let mut reader = BufReader::with_capacity(BUF_SIZE, file);

    let mut offsets: Vec<u64> = Vec::new();
    let mut chunk_has_bytes = false;
    let mut file_pos: u64 = 0;

    // Carry-over to avoid missing delimiter when parsing chunks
    let mut carry: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; BUF_SIZE];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        chunk_has_bytes = true;

        let carry_len = carry.len();
        let chunk_start_pos = file_pos - carry_len as u64;
        let mut combined = std::mem::take(&mut carry);
        combined.extend_from_slice(&buf[..n]);

        // Track loc in case chunk contains multiple records
        let mut search_from = 0usize;
        while let Some(sep_idx) = finder.find(&combined[search_from..]) {
            let sep_abs = search_from + sep_idx;
            let prev_ok = if sep_abs == 0 {
                chunk_start_pos == 0
            } else {
                combined[sep_abs - 1] == b'\n'
            };
            let after = sep_abs + delimiter.len();
            let next_record_offset_in_combined = if after >= combined.len() {
                None
            } else if combined[after] == b'\n' && prev_ok {
                Some(after + 1)
            } else if combined[after] == b'\r'
                && after + 1 < combined.len()
                && combined[after + 1] == b'\n'
                && prev_ok
            {
                Some(after + 2)
            } else if prev_ok && after == combined.len() {
                None
            } else {
                search_from = sep_abs + 1;
                continue;
            };

            match next_record_offset_in_combined {
                Some(next_off) => {
                    let abs_off = chunk_start_pos + next_off as u64;
                    offsets.push(abs_off);
                    search_from = next_off;
                }
                None => break,
            }
        }

        let keep = delimiter.len() + 2;
        if combined.len() > keep {
            carry = combined[combined.len() - keep..].to_vec();
        } else {
            carry = combined;
        }

        file_pos += n as u64;
    }

    // Return error if no bytes in file
    if !chunk_has_bytes {
        return Err(anyhow!("File does not contain any readable records."));
    }

    // Construct the final byte offsets Vec
    let mut final_bytes_vec = Vec::with_capacity(offsets.len() + 1);
    // Make sure the whole thing starts with 0
    final_bytes_vec.push(0u64);
    // Account for potential last $$$$ in files:
    for off in offsets {
        if off < total_file_bytes {
            final_bytes_vec.push(off);
        }
    }
    Ok(final_bytes_vec)
}
