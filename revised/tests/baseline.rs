#![allow(dead_code, unused_imports, clippy::all)]
mod support {
    include!("support/original.rs");
    #[test]
    #[ignore = "Writes a 288 MB baseline dataset; run explicitly in release mode"]
    fn baseline_throughput() {
        let root = std::path::PathBuf::from(
            std::env::var("MIPOINT_BENCH_OUTPUT").expect("Set a new MIPOINT_BENCH_OUTPUT folder"),
        );
        assert!(!root.exists(), "Use a new folder");
        let cfg = Config {
            output: root.clone(),
            files: 60,
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
        };
        let start = std::time::Instant::now();
        let bytes = create_tree(&root, &cfg, &mut Rng::new(cfg.seed), |_, _, _| {}).unwrap();
        let elapsed = start.elapsed().as_secs_f64();
        println!(
            "BASELINE files=60 bytes={bytes} seconds={elapsed:.6} MB/s={:.3}",
            bytes as f64 / 1e6 / elapsed
        );
    }
}
