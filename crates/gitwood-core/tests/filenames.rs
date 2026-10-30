//! Stage 0 platform baseline: real Git round-trips names without line-based parsing.
//! This fixture does not implement or substitute for the future production Git runner.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

struct Fixture(TempDir);

impl Fixture {
    fn new() -> Self {
        let fixture = Self(tempfile::tempdir().expect("create unique system temporary repository"));
        fixture.git(&["init", "--quiet", "--template="]);
        fixture
    }

    fn git(&self, arguments: &[&str]) -> Vec<u8> {
        let output = Command::new("git")
            .args(["--no-optional-locks", "-c", "core.fsmonitor=false"])
            .args(arguments)
            .current_dir(self.0.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .output()
            .expect("Git must be installed for the stage 0 filename baseline");
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    fn round_trip(&self, names: &[&str]) {
        for name in names {
            fs::write(self.0.path().join(name), b"Gitwood filename fixture\n")
                .expect("write fixture file");
            self.git(&["add", "--", name]);
            let recorded = self.git(&["ls-files", "--cached", "-z", "--", name]);
            let mut expected = name.as_bytes().to_vec();
            expected.push(0);
            assert_eq!(
                recorded, expected,
                "filename must round-trip with NUL framing"
            );
        }
    }
}

#[test]
fn portable_filenames_round_trip() {
    Fixture::new().round_trip(&["中文 空格.txt", "-leading-dash.txt"]);
}

// Windows forbids control characters (including tab/newline) in filenames.
#[cfg(not(windows))]
#[test]
fn unix_control_characters_round_trip() {
    Fixture::new().round_trip(&["tab\tname.txt", "line\nname.txt"]);
}
