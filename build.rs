use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir.join("scripts").join("validate.sh");

    println!("cargo:rerun-if-changed=scripts/validate.sh");
    println!("cargo:rerun-if-changed=.git/HEAD");

    let bytes = fs::read(&script_path).unwrap_or_else(|error| {
        panic!("build.rs: read {}: {error}", script_path.display());
    });
    let hex = format!("{:x}", Sha256::digest(&bytes));

    println!("cargo:rustc-env=VALIDATE_SH_SHA={hex}");
    println!(
        "cargo:rustc-env=RUNE_BUILD_COMMIT={}",
        build_commit(&manifest_dir)
    );
    println!("cargo:rustc-env=RUNE_BUILD_TIME={}", build_time());
}

fn build_commit(manifest_dir: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(manifest_dir)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|commit| commit.trim().to_owned())
        .filter(|commit| !commit.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn build_time() -> String {
    let unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format_utc(unix_seconds)
}

fn format_utc(unix_seconds: u64) -> String {
    let days = i64::try_from(unix_seconds / 86_400).expect("build time exceeds i64 days");
    let seconds = unix_seconds % 86_400;
    let hour = seconds / 3_600;
    let minute = seconds % 3_600 / 60;
    let second = seconds % 60;
    let (year, month, day) = civil_from_days(days);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

// Convert days since the Unix epoch to a Gregorian calendar date.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted_days = days + 719_468;
    let era = if shifted_days >= 0 {
        shifted_days
    } else {
        shifted_days - 146_096
    } / 146_097;
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }

    (year, month, day)
}
