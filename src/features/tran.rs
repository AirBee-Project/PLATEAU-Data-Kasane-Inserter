use std::path::{Path, PathBuf};
use crate::error::AppError;

/// tran ディレクトリ内の各GMLファイルを並列処理します
pub async fn process_directory(dir: PathBuf) -> Result<(), AppError> {
    let mut tasks = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "gml") {
                let handle = tokio::spawn(async move {
                    if let Err(e) = process_gml_file(&path).await {
                        eprintln!(
                            "[tran] ファイル {:?} の処理中にエラーが発生しました: {}",
                            path.file_name().unwrap_or_default(),
                            e
                        );
                    }
                });
                tasks.push(handle);
            }
        }
    }
    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}

/// 各GMLファイルの読み込みとダミー処理 (文字数カウント)
async fn process_gml_file(path: &Path) -> Result<(), AppError> {
    let content = tokio::fs::read_to_string(path).await?;
    let char_count = content.chars().count();
    println!(
        "[tran] GMLファイル: {:?} (文字数: {})",
        path.file_name().unwrap_or_default(),
        char_count
    );
    Ok(())
}
