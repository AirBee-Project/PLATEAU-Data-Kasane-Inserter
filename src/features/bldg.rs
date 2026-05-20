use crate::error::AppError;
use std::path::{Path, PathBuf};

/// 各GMLファイルの処理
async fn process_gml_file(path: &Path) -> Result<(), AppError> {
    let content = tokio::fs::read_to_string(path).await?;

    let mut ids = Vec::new();
    // 時空間IDへの変換
    match nazori::plateau::bldg(&content, 25, 0.0) {
        Ok(v) => {
            for ele in v {
                match ele {
                    Ok(v) => {
                        ids.push(v.to_string());
                    }
                    Err(e) => {
                        tracing::error!("ファイル {:?} の時空間ID変換エラーが発生しました: {}", path.display(), e);
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("ファイル {:?} のデータパースエラーが発生しました: {}", path.display(), e);
        }
    }

    let output_path = path.with_extension("txt");
    let output_content = ids.join(",");
    tokio::fs::write(&output_path, output_content).await?;
    tracing::info!(
        "結果の時空間IDを {:?} に保存しました (件数: {})",
        output_path,
        ids.len()
    );
    Ok(())
}

/// bldg ディレクトリ内の各GMLファイルを並列処理します
pub async fn process_directory(
    dir: PathBuf,
    pref_city: &str,
    mp: std::sync::Arc<indicatif::MultiProgress>,
) -> Result<(), AppError> {
    let mut gml_paths = Vec::new();
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "gml") {
            gml_paths.push(path);
        }
    }

    let total_files = gml_paths.len();
    if total_files == 0 {
        return Ok(());
    }

    let pb = mp.add(indicatif::ProgressBar::new(total_files as u64));
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({eta}) - {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(format!("{} [bldg] 変換中", pref_city));

    let mut futures = Vec::new();
    for path in gml_paths {
        let pb_clone = pb.clone();
        futures.push(async move {
            let res = process_gml_file(&path).await;
            pb_clone.inc(1);
            res
        });
    }

    futures::future::try_join_all(futures).await?;
    pb.finish_with_message(format!("{} [bldg] 変換完了", pref_city));
    Ok(())
}
