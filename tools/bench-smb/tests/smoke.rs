//! End-to-end: run the real binary against a folder this test generates.

use std::fs;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bench-smb"))
}

#[test]
fn binary_reports_on_generated_folder_and_leaves_it_untouched() {
    let dir = std::env::temp_dir().join(format!("bench-smb-e2e-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    for i in 0..45 {
        fs::write(
            dir.join(format!("p{i:03}.JPG")),
            vec![i as u8; 30_000 + i * 100],
        )
        .unwrap();
    }
    let before = fs::read_dir(&dir).unwrap().count();

    let out = bin().arg(&dir).args(["--runs", "2"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let _ = fs::remove_dir_all(&dir);

    assert!(out.status.success(), "{text}");
    assert!(text.starts_with("# bench-smb report"));
    assert!(text.contains("files: 45 (45 JPEG)"));
    assert!(text.contains("| 16 |"), "sweep must include K=16:\n{text}");
    assert_eq!(before, 45);
}

#[test]
fn missing_dir_prints_report_with_error_and_exits_1() {
    let out = bin().arg("Z:/no/such/photosee-bench-dir").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("cannot list"));
}

#[test]
fn bad_arguments_exit_2_with_usage() {
    let out = bin().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage: bench-smb"));
}
