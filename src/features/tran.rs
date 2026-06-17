use crate::config::KasaneConfig;
use crate::error::AppError;
use crate::kasane::client::KasaneClient;
use crate::kasane::models::SpatialId;
use futures::stream::{self, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 各GMLファイルを変換し、Kasane の交通テーブルへ挿入します。
async fn process_gml_file(
    path: &Path,
    client: &Arc<KasaneClient>,
    cfg: &Arc<KasaneConfig>,
) -> Result<(), AppError> {
    let content = tokio::fs::read_to_string(path).await?;
    let path_clone = path.to_path_buf();
    let zoom = cfg.max_zoom;
    let limit = cfg.debug_limit_per_file;

    // パース・空間ID変換はCPUバウンドのため spawn_blocking で実行する
    let spatial_ids = tokio::task::spawn_blocking(move || {
        let mut ids = Vec::new();
        match nazori::plateau::tran(&content, zoom, 0.0) {
            Ok(v) => {
                for ele in v {
                    // デバッグ用: ファイルあたり先頭N件で打ち切る
                    if let Some(l) = limit {
                        if ids.len() >= l {
                            break;
                        }
                    }
                    match ele {
                        Ok(id) => ids.push(SpatialId::from(&id)),
                        Err(e) => {
                            tracing::error!("時空間ID変換エラー {:?}: {}", path_clone.display(), e)
                        }
                    }
                }
            }
            Err(e) => tracing::error!("パースエラー {:?}: {}", path_clone.display(), e),
        }
        ids
    })
    .await?;

    if spatial_ids.is_empty() {
        return Ok(());
    }

    let total = spatial_ids.len();

    // バッチに分割し、並列で挿入する
    let batches: Vec<Vec<SpatialId>> = spatial_ids
        .chunks(cfg.insert_batch)
        .map(|c| c.to_vec())
        .collect();

    let results: Vec<Result<(), AppError>> = stream::iter(batches.into_iter().map(|batch| {
        let client = client.clone();
        let cfg = cfg.clone();
        async move {
            client
                .insert_data(
                    &cfg.database,
                    &cfg.tran_table,
                    serde_json::json!(true),
                    &batch,
                    cfg.zoom_policy,
                )
                .await
        }
    }))
    .buffer_unordered(cfg.insert_concurrency)
    .collect()
    .await;

    for r in results {
        r?;
    }

    tracing::info!("挿入完了: {:?} ({}件)", path, total);
    Ok(())
}

/// tran ディレクトリ内の各GMLファイルを直列処理します
pub async fn process_directory(
    dir: PathBuf,
    pref_city: &str,
    mp: std::sync::Arc<indicatif::MultiProgress>,
    client: Arc<KasaneClient>,
    cfg: Arc<KasaneConfig>,
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
    pb.set_message(format!("{} [tran] 挿入中", pref_city));

    for path in gml_paths {
        process_gml_file(&path, &client, &cfg).await?;
        pb.inc(1);
    }

    pb.finish_with_message(format!("{} [tran] 挿入完了", pref_city));
    Ok(())
}
