use std::path::PathBuf;

use fin_sdk::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

fn main() {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("bench-fixtures")
                .join("benchmark-runtime")
        });

    match materialize_fixture_home(&output, &FixtureBuildOptions::default()) {
        Ok(materialized) => {
            serde_json::to_writer_pretty(std::io::stdout(), &materialized)
                .expect("write materialized fixture json");
            println!();
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
