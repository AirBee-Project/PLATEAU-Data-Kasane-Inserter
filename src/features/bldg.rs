use crate::error::AppError;
use std::path::{Path, PathBuf};

/// 各GMLファイルの処理
async fn process_gml_file(path: &Path) -> Result<(), AppError> {
    let content = tokio::fs::read_to_string(path).await?;
    let path_clone = path.to_path_buf();

    let ids = tokio::task::spawn_blocking(move || {
        let mut ids = Vec::new();
        match nazori::plateau::bldg(&content, 25, 0.0) {
            Ok(v) => {
                for ele in v {
                    match ele {
                        Ok(id) => ids.push(id.to_string()),
                        Err(e) => tracing::error!("時空間ID変換エラー {:?}: {}", path_clone.display(), e),
                    }
                }
            }
            Err(e) => tracing::error!("パースエラー {:?}: {}", path_clone.display(), e),
        }
        ids
    })
    .await?;

    let output_path = path.with_extension("txt");
    tokio::fs::write(&output_path, ids.join(",")).await?;
    tracing::info!("保存完了: {:?} ({}件)", output_path, ids.len());
    
    Ok(())
}

/// bldg ディレクトリ内の各GMLファイルを直列処理します
pub async fn process_directory(
    dir: PathBuf,
    pref_city: &str,
    mp: std::sync::Arc<indicatif::MultiProgress>,
) -> Result<(), AppError> {
    let gml_paths: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "gml"))
        .collect();

    if gml_paths.is_empty() {
        return Ok(());
    }

    let pb = mp.add(indicatif::ProgressBar::new(gml_paths.len() as u64));
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({eta}) - {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(format!("{} [bldg] 変換中", pref_city));

    for path in gml_paths {
        process_gml_file(&path).await?;
        pb.inc(1);
    }

    pb.finish_with_message(format!("{} [bldg] 変換完了", pref_city));
    Ok(())
}
