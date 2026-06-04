use anyhow::{Context, Result, anyhow};
use memchr::memchr;
use std::{collections::HashSet, fs::File, path::PathBuf};

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
    pub fn read_record(&self, idx: usize) -> Result<String> {
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
    pub fn read_title(&self, idx: usize) -> Result<String> {
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

    /// Collect all the titles from a file
    pub fn titles(&self) -> Result<Vec<String>> {
        (0..self.byte_offsets.len())
            .map(|i| self.read_title(i))
            .collect()
    }

    /// Return the unique titles in the sdf file
    pub fn unique(&self) -> Result<Vec<String>> {
        let mut unique: HashSet<String> = HashSet::with_capacity(self.byte_offsets.len());
        for i in 0..self.byte_offsets.len() {
            unique.insert(self.read_title(i)?);
        }
        Ok(unique.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_FILE: &str = "./data/tiny_test_set.sdf";

    #[test]
    fn file_reading() {
        let sdf = SDFile::open(TEST_FILE.into()).unwrap();
        assert_eq!(sdf.len(), 4);
        assert_eq!(sdf.is_empty(), false);
        assert_eq!(sdf.get_record_loc(0).unwrap(), (0, 5639));

        let lines: Vec<String> = sdf
            .read_record(0)
            .unwrap()
            .lines()
            .map(String::from)
            .collect();

        assert_eq!(lines[0], "S388-0404");
        assert_eq!(lines[1], "                    3D");
        assert_eq!(lines[2], " Schrodinger Suite 2021-4.");
    }

    #[test]
    fn titles() {
        use std::fs::File;
        use std::io::Write;

        let sdf = SDFile::open(TEST_FILE.into()).unwrap();
        let true_titles = vec!["S388-0404", "S395-0132", "T655-0622", "T655-0634"];
        // test individual read_title calls
        let titles_from_read_title: Vec<String> = {
            (0..4)
                .map(|i| sdf.read_title(i))
                .filter_map(Result::ok)
                .collect()
        };
        assert_eq!(titles_from_read_title.len(), 4);
        assert_eq!(titles_from_read_title, true_titles);

        // test SDFile::titles
        let titles_from_titles = sdf.titles().unwrap();
        assert_eq!(titles_from_titles.len(), 4);
        assert_eq!(titles_from_titles, true_titles);

        // test SDFile::unique
        let all_unique = sdf.unique().unwrap();
        assert_eq!(all_unique.len(), 4);

        assert_eq!(
            all_unique.into_iter().collect(),
            true_titles.into_iter().collect()
        );

        // Create a temp file with duplicate titles
        let tmp_path = std::env::temp_dir().join("duplicate.sdf");
        let mut f = File::create(&tmp_path).unwrap();
        f.write_all(b"Mol_A\nbody\n$$$$\n").unwrap();
        f.write_all(b"Mol_B\nbody\n$$$$\n").unwrap();
        f.write_all(b"Mol_A\nbody\n$$$$\n").unwrap(); // Duplicated!

        let dupe_sdf = SDFile::open(tmp_path.clone()).unwrap();

        let dupe_titles_all = dupe_sdf.unique().unwrap();
        let dupe_titles_unique = dupe_sdf.unique().unwrap();

        assert_eq!(dupe_titles_all.len(), 3);
        assert_eq!(dupe_titles_unique.len(), 2);

        // Because the order of items coming out of a HashSet is non-deterministic,
        // we convert both our expected array and the result back into HashSets for comparison.
        let expected: HashSet<String> = vec!["Mol_A".to_string(), "Mol_B".to_string()]
            .into_iter()
            .collect();

        let actual: HashSet<String> = unique_titles.into_iter().collect();

        assert_eq!(actual, expected);

        // 6. Clean up the temp file so we don't litter the OS
        std::fs::remove_file(tmp_path).unwrap();
    }
}
