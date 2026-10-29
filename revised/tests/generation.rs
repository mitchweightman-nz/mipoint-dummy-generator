use mipoint_dummy_generator::engine::{self, Config, Format};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
fn config(output: PathBuf) -> Config {
    Config {
        output,
        files: 17,
        max_depth: 7,
        seed: 4242,
        formats: vec![
            Format::Docx,
            Format::Xlsx,
            Format::Pptx,
            Format::Csv,
            Format::Rtf,
            Format::Txt,
        ],
        inventory: true,
        average_mb: 0.12,
        minimum_mb: 0.1,
        maximum_mb: 0.2,
    }
}
fn temp(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mipoint-test-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn contents(root: &std::path::Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        p: &std::path::Path,
        out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>,
    ) {
        for e in std::fs::read_dir(p).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out)
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}
fn cleanup(p: &std::path::Path) {
    let resolved = p.canonicalize().unwrap();
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert!(resolved.starts_with(temp));
    assert!(p
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("mipoint-test-"));
    std::fs::remove_dir_all(p).unwrap();
}
fn cleanup_checkpoint(directory: &std::path::Path) {
    let resolved = directory.canonicalize().unwrap();
    let base = engine::checkpoint_directory().canonicalize().unwrap();
    assert_eq!(resolved.parent(), Some(base.as_path()));
    assert!(resolved
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("Dataset-4242-"));
    std::fs::remove_dir_all(resolved).unwrap();
}
#[test]
fn reproducible_inventory_depth_and_no_overwrite() {
    let p = temp("repeat");
    let cfg = config(p.clone());
    let cancel = Arc::new(AtomicBool::new(false));
    let a = engine::generate(&cfg, &cancel, |_| {}).unwrap();
    let first = contents(&a.output);
    let b = engine::generate(&cfg, &cancel, |_| {}).unwrap();
    assert_ne!(a.output, b.output);
    assert_eq!(first, contents(&b.output));
    assert_eq!(a.bytes, 17 * 120_000);
    assert_eq!(a.completed, 17);
    assert!(!a.cancelled);
    for (path, data) in &first {
        if path.extension().is_some_and(|x| x != "csv")
            || path.file_name().unwrap() != "Structure Inventory.csv"
        {
            assert!((100_000..=200_000).contains(&data.len()));
            let depth = path.components().count() - 1;
            assert!((3..=7).contains(&depth));
        }
    }
    assert_eq!(
        std::fs::read_to_string(a.output.join("Structure Inventory.csv"))
            .unwrap()
            .lines()
            .count(),
        18
    );
    assert_eq!(first, contents(&a.output));
    cleanup_checkpoint(&a.checkpoint);
    cleanup_checkpoint(&b.checkpoint);
    cleanup(&p);
}
#[test]
fn cancellation_keeps_only_completed_files_and_inventory() {
    let p = temp("cancel");
    let cfg = config(p.clone());
    let cancel = Arc::new(AtomicBool::new(false));
    let report = engine::generate(&cfg, &cancel, |p| {
        if p.completed == 3 {
            cancel.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert_eq!(report.completed, 3);
    assert!(report.cancelled);
    assert_eq!(contents(&report.output).len(), 4);
    assert_eq!(
        std::fs::read_to_string(report.output.join("Structure Inventory.csv"))
            .unwrap()
            .lines()
            .count(),
        4
    );
    cleanup_checkpoint(&report.checkpoint);
    cleanup(&p);
}
#[test]
fn invalid_config_never_creates_output() {
    let p = temp("invalid");
    let mut cfg = config(p.clone());
    cfg.average_mb = 0.5;
    assert!(cfg.validate().unwrap_err().contains("cannot fit"));
    assert!(engine::generate(&cfg, &Arc::new(AtomicBool::new(false)), |_| {}).is_err());
    assert!(!p.exists());
    cfg.average_mb = 0.12;
    cfg.minimum_mb = f64::NAN;
    assert!(cfg.validate().is_err());
    cfg.minimum_mb = 0.1;
    cfg.formats.clear();
    assert!(cfg.validate().is_err());
    cfg.formats.push(Format::Txt);
    cfg.max_depth = 0;
    assert!(cfg.validate().is_err());
}
#[test]
fn depth_one_and_inventory_off() {
    let p = temp("shallow");
    let mut cfg = config(p.clone());
    cfg.max_depth = 1;
    cfg.files = 1;
    cfg.inventory = false;
    cfg.minimum_mb = 0.12;
    cfg.maximum_mb = 0.12;
    let report = engine::generate(&cfg, &Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    let files = contents(&report.output);
    assert_eq!(files.len(), 1);
    assert_eq!(files.keys().next().unwrap().components().count(), 2);
    assert_eq!(report.bytes, 120_000);
    cleanup_checkpoint(&report.checkpoint);
    cleanup(&p);
}
#[test]
fn blocked_destination_reports_error() {
    let p = temp("blocked");
    std::fs::write(&p, b"keep this file").unwrap();
    let cfg = config(p.clone());
    let report = engine::generate(&cfg, &Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert!(report.paused);
    cleanup_checkpoint(&report.checkpoint);
    assert_eq!(std::fs::read(&p).unwrap(), b"keep this file");
    std::fs::remove_file(&p).unwrap();
}
#[test]
#[ignore = "Writes a 288 MB dataset; run explicitly in release mode"]
fn revised_throughput() {
    let p = PathBuf::from(std::env::var("MIPOINT_BENCH_OUTPUT").expect("Set MIPOINT_BENCH_OUTPUT"));
    assert!(!p.exists(), "Use a new folder");
    let mut cfg = config(p);
    cfg.files = 60;
    cfg.average_mb = 4.8;
    cfg.minimum_mb = 1.0;
    cfg.maximum_mb = 15.0;
    let start = Instant::now();
    let report = engine::generate(&cfg, &Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    let elapsed = start.elapsed().as_secs_f64();
    assert_eq!(report.bytes, 288_000_000);
    println!(
        "REVISED files={} bytes={} seconds={elapsed:.6} MB/s={:.3} output={}",
        report.completed,
        report.bytes,
        report.bytes as f64 / 1e6 / elapsed,
        report.output.display()
    );
}
