use tempfile::tempdir;

use fin_sdk::testing::fixture::{
    FixtureBuildOptions, canonical_fixture_snapshot, materialize_fixture_home,
};

#[test]
fn repeated_materialization_is_identical() {
    let left = tempdir().expect("left tempdir");
    let right = tempdir().expect("right tempdir");

    let left_fixture = materialize_fixture_home(left.path(), &FixtureBuildOptions::default())
        .expect("left fixture");
    let right_fixture = materialize_fixture_home(right.path(), &FixtureBuildOptions::default())
        .expect("right fixture");

    assert_eq!(left_fixture.stats, right_fixture.stats);
    assert_eq!(
        canonical_fixture_snapshot(&left_fixture.paths.db_path).expect("left snapshot"),
        canonical_fixture_snapshot(&right_fixture.paths.db_path).expect("right snapshot"),
    );
}
