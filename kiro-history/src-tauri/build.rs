fn main() {
    // dist/ 内の全ファイルを個別に監視（ディレクトリ単体ではCargoが変更を検知しないため）
    let dist_dir = std::path::Path::new("../dist");
    if dist_dir.exists() {
        register_dir_for_rerun(dist_dir);
    }
    tauri_build::build()
}

fn register_dir_for_rerun(dir: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                register_dir_for_rerun(&path);
            } else {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
