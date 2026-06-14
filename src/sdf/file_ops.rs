use anyhow::{Result, anyhow};
use std::{fs::File, io::BufReader, path::PathBuf};

pub fn split_file(file_reader: &mut BufReader<File>, sizes: Vec<usize>) -> Result<Vec<PathBuf>> {}
