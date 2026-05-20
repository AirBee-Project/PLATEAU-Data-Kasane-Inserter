use std::path::{Path, PathBuf};
use crate::error::AppError;
use tracing::debug;

/// tran ディレクトリ内の各GMLファイルを並列処理します
pub async fn process_directory(dir: PathBuf) -> Result<(), AppError> {
    let mut futures = Vec::new();
    let entries = std::fs::read_dir(dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "gml") {
            futures.push(async move {
                process_gml_file(&path).await
            });
        }
    }

    futures::future::try_join_all(futures).await?;
    Ok(())
}

/// 各GMLファイルの読み込みとダミー処理 (文字数カウント)
async fn process_gml_file(path: &Path) -> Result<(), AppError> {
    let content = tokio::fs::read_to_string(path).await?;
    let char_count = content.chars().count();
    debug!(
        "[tran] GMLファイル: {:?} (文字数: {})",
        path.file_name().unwrap_or_default(),
        char_count
    );
    Ok(())
}
