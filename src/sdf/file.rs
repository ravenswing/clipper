use anyhow::{Context, Result, anyhow};
use memchr::memchr;
use std::{fs::File, path::PathBuf};

use crate::sdf::io::{pos_read_bytes, scan_offsets, trim_delim};

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

    /// Read specific bytes from the file, starting at `offset`.
    pub fn read_bytes(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        let len = len as usize;
        let mut buf = vec![0u8; len];
        let file = self
            .file
            .as_ref()
            .with_context(|| "SDFile is not open to read bytes")?;

        pos_read_bytes(file, offset, &mut buf)
            .with_context(|| format!("read of {} bytes at offset {} failed", len, offset))?;

        Ok(buf)
    }

    /// Get the byte offset range for the record at `idx`.
    /// Returns `(start, end)`.
    pub fn get_record_loc(&self, idx: usize) -> Result<(u64, u64)> {
        // Get the start from offsets if there is one
        let start = *self
            .byte_offsets
            .get(idx)
            .ok_or_else(|| anyhow!("record index out of range: {}", idx))?;
        // Get the end if possible, default to the end of the file if not
        let end = self
            .byte_offsets
            .get(idx + 1)
            .copied()
            .unwrap_or(self.byte_len);
        Ok((start, end))
    }

    // Read a single record from a file to owned `String`.
    fn read_record(&self, idx: usize) -> Result<String> {
        let (start, end) = self.get_record_loc(idx)?;
        let buf = self.read_bytes(start, end - start)?;
        // Note: Use lossy decoding so legacy Latin-1 / ISO-8859-1 SDFs don't crashthe parser.
        // Invalid bytes become U+FFFD REPLACEMENT CHARACTER.
        let mut text = String::from_utf8_lossy(&buf).into_owned();
        trim_delim(&mut text);
        Ok(text)
    }

    /// Read only the bytes needed to extract the title, without loading the
    /// full record into memory.
    fn read_title(&self, idx: usize) -> Result<String> {
        let (start, end) = self.get_record_loc(idx)?;
        // only read up to a max of 1kB for the title
        let len = 1024u64.min(end - start);
        let buf = self.read_bytes(start, len)?;
        // Search for the end of the line char
        let eol_loc = memchr(b'\n', &buf).unwrap_or(buf.len());
        // Convert line and trim new line etc before returning
        let line = String::from_utf8_lossy(&buf[..eol_loc]);
        Ok(line.trim_end_matches('\r').trim_end().to_string())
    }
}
