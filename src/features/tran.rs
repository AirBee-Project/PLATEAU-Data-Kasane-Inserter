use crate::error::AppError;
use std::path::{Path, PathBuf};

/// tran ディレクトリ内の各GMLファイルを並列処理します
pub async fn process_directory(dir: PathBuf) -> Result<(), AppError> {
    let mut tasks = Vec::new();
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "gml") {
            let handle = tokio::spawn(async move { process_gml_file(&path).await });
            tasks.push(handle);
        }
    }

    let mut first_error = None;
    for t in tasks {
        match t.await {
            Ok(Err(e)) => {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(AppError::Join(e));
                }
            }
            Ok(Ok(())) => {}
        }
    }

    if let Some(e) = first_error {
        return Err(e);
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
