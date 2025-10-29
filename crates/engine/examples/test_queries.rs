extern crate engine;
use engine::Engine;

use env_logger;
use log::{error, info};
use std::{fs, path::PathBuf, sync::mpsc, thread, time::Instant};

#[derive(Debug, Clone)]
struct FileItem {
    name: String,
    path: String,
}

#[derive(Debug)]
struct TestResult {
    name: String,
    path: String,
    ok: bool,
    error: Option<String>,
    elapsed_ms: u128,
}

fn main() {
    // RUST_LOG=info ./your-binary  to see logs
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let dir = "test queries";
    let mut files: Vec<FileItem> = Vec::new();

    // Collect files in the directory (non-recursive). Filter to regular files and (optionally) extension.
    match fs::read_dir(dir) {
        Ok(read_dir) => {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                // Optional: limit to certain extensions (uncomment if you want)
                // match path.extension().and_then(|e| e.to_str()) {
                //     Some("qry") | Some("txt") => {},
                //     _ => continue,
                // }

                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("<unknown>")
                    .to_string();

                files.push(FileItem {
                    name,
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
        Err(e) => {
            eprintln!("Failed to read dir '{}': {}", dir, e);
            std::process::exit(1);
        }
    }

    if files.is_empty() {
        println!("No test scripts found in '{}'.", dir);
        return;
    }

    info!("Discovered {} test script(s)", files.len());

    // Channel to collect results
    let (tx, rx) = mpsc::channel::<TestResult>();

    // Spawn one thread per file
    let mut joins = Vec::with_capacity(files.len());
    for file in files {
        let tx = tx.clone();
        let FileItem { name, path } = file;
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let res = run_single_test(&name, &path);
            let elapsed_ms = start.elapsed().as_millis();
            match res {
                Ok(_) => {
                    info!("[PASS] {}", name);
                    let _ = tx.send(TestResult {
                        name,
                        path,
                        ok: true,
                        error: None,
                        elapsed_ms,
                    });
                }
                Err(e) => {
                    error!("[FAIL] {} -> {}", name, e);
                    let _ = tx.send(TestResult {
                        name,
                        path,
                        ok: false,
                        error: Some(e),
                        elapsed_ms,
                    });
                }
            }
        });
        joins.push(handle);
    }

    // Drop the extra sender so rx will close after workers finish
    drop(tx);

    // Collect all results
    let mut results: Vec<TestResult> = Vec::new();
    while let Ok(r) = rx.recv() {
        results.push(r);
    }

    // Join threads
    for j in joins {
        let _ = j.join();
    }

    // Print summary
    println!("\n==== Test Summary ====");
    let mut passed = 0usize;
    let mut failed = 0usize;
    for r in &results {
        if r.ok {
            println!("[PASS] {:<30} ({:>5} ms)  {}", r.name, r.elapsed_ms, r.path);
            passed += 1;
        } else {
            println!(
                "[FAIL] {:<30} ({:>5} ms)  {}  :: {}",
                r.name,
                r.elapsed_ms,
                r.path,
                r.error.as_deref().unwrap_or("unknown error")
            );
            failed += 1;
        }
    }
    println!("-----------------------");
    println!(
        "Total: {}  |  Passed: {}  |  Failed: {}",
        results.len(),
        passed,
        failed
    );
}

/// Runs a single file through Engine.
/// Returns Ok(()) on success; Err(message) on failure.
fn run_single_test(name: &str, path: &str) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|e| format!("read file failed: {}", e))?;

    // Pass the *content* because Engine::new takes source when is_src_input = Some(true)
    let mut engine = Engine::new(&content, "127.0.0.1:7000", Some(true))
        .map_err(|e| format!("engine init failed: {}", e))?;

    info!("Running Engine for {name}");
    let query = engine.query();
    info!("QUERY : {:#?}", query);

    engine
        .run()
        .map_err(|e| format!("Engine run failed: {}", e))?;

    // If you want to assert something about the output, you can inspect it here:
    // if let Some(out) = engine.get_output() { ... }

    Ok(())
}
