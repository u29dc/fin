use std::path::PathBuf;

use fin_sdk::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

fn main() {
    let mut args = std::env::args().skip(1);
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("bench-fixtures")
            .join("benchmark-runtime")
    });
    let transaction_scale = args
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);

    match materialize_fixture_home(
        &output,
        &FixtureBuildOptions {
            transaction_scale,
            ..FixtureBuildOptions::default()
        },
    ) {
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
