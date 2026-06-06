use anyhow::Result;

pub mod sdf;
use sdf::SDFile;

fn main() -> Result<()> {
    println!("Welcome to Clipper");

    let test_file = "./data/tiny_test_set.sdf";

    let sdf = SDFile::open(test_file.into()).unwrap();

    let len = sdf.len();
    println!("Number of entries in file: {len}");

    println!(
        "Location of first entry: {:?}",
        sdf.get_record_loc(0).unwrap()
    );

    let lines: Vec<String> = sdf
        .read_record(0)
        .unwrap()
        .lines()
        .map(String::from)
        .collect();
    println!("Location of first entry: {:?}", lines[0]);
    println!("Location of first entry: {:?}", lines[1]);
    println!("Location of first entry: {:?}", lines[2]);

    let titles: Vec<String> = {
        (0..4)
            .map(|i| sdf.read_title(i))
            .filter_map(Result::ok)
            .collect()
    };
    println!("Location of first entry: {:?}", titles);

    Ok(())
}
