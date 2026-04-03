use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    println!("Welcome to Clipper");

    let sdf = fs::read_to_string("./data/tiny_test_set.sdf")?;

    let mols: Vec<Vec<&str>> = sdf
        .trim()
        .split("$$$$")
        .map(|x| x.trim().lines().collect())
        .collect();

    println!("{:?}", mols);
    println!("{:?}", mols.len());

    Ok(())
}
