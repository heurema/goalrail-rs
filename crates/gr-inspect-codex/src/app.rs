use std::{path::Path, time::Duration};

use gr_inspect_core::{ProcessOutput, run_bounded};

pub(crate) const CODEX_APP_INFO_PLIST: &str = "/Applications/Codex.app/Contents/Info.plist";
// plutil reads a local bundle plist; one second bounds a hung helper without affecting the verdict.
const APP_PROBE_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) fn probe_app_version(info_plist: &Path) -> Option<String> {
    if !info_plist.is_file() {
        return None;
    }

    let output = run_bounded(
        "/usr/bin/plutil",
        &[
            "-extract",
            "CFBundleShortVersionString",
            "raw",
            "-o",
            "-",
            info_plist.to_str()?,
        ],
        APP_PROBE_TIMEOUT,
    )
    .ok()?;

    app_version_text(&output)
}

fn app_version_text(output: &ProcessOutput) -> Option<String> {
    if output.timed_out || output.exit_code != Some(0) {
        return None;
    }

    let text = std::str::from_utf8(&output.stdout).ok()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(stdout: &[u8], exit_code: Option<i32>, timed_out: bool) -> ProcessOutput {
        ProcessOutput {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code,
            duration: Duration::ZERO,
            timed_out,
        }
    }

    #[test]
    fn parses_a_trimmed_app_version() {
        assert_eq!(
            app_version_text(&output(b"26.715.61943\n", Some(0), false)),
            Some("26.715.61943".to_owned())
        );
    }

    #[test]
    fn rejects_missing_invalid_or_timed_out_app_version_output() {
        let cases = [
            output(b"", Some(0), false),
            output(b"26.715.61943", Some(1), false),
            output(b"26.715.61943", Some(0), true),
            output(&[0xff], Some(0), false),
        ];

        for output in cases {
            assert_eq!(app_version_text(&output), None);
        }
    }

    #[test]
    fn returns_none_when_app_metadata_is_missing() {
        let path = std::env::temp_dir().join(format!(
            "goalrail-codex-app-missing-{}.plist",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        assert_eq!(probe_app_version(&path), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn probes_an_app_version_from_a_plist_file() {
        let path =
            std::env::temp_dir().join(format!("goalrail-codex-app-{}.plist", std::process::id()));
        std::fs::write(
            &path,
            br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleShortVersionString</key><string>fixture-version</string></dict></plist>
"#,
        )
        .expect("fixture plist should be writable");

        let version = probe_app_version(&path);
        let _ = std::fs::remove_file(&path);

        assert_eq!(version, Some("fixture-version".to_owned()));
    }
}
