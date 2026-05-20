use plateau_data_kasane_inserter::features::{bldg, tran};
use plateau_data_kasane_inserter::list::CityList;
use plateau_data_kasane_inserter::scheduler::Scheduler;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() {
    // tracing-subscriber を初期化 (RUST_LOG 環境変数を参照します)
    tracing_subscriber::fmt::init();

    // 1. 都市の一覧を取得 (デフォルトで "cache/plateau_cache.json" をキャッシュに使用)
    let city_list = match CityList::new().await {
        Ok(list) => list,
        Err(e) => {
            tracing::error!("都市一覧の取得に失敗しました: {}", e);
            return;
        }
    };

    // 件数制限を設定 (デバッグが容易になるよう、最初の3件のみに制限)
    let city_list = city_list.take(3);
    let cities_to_process = city_list.cities().to_vec();
    tracing::info!("処理対象の都市数: {}", cities_to_process.len());

    // 2. スケジューラの初期化 (並列実行数: 2, キャッシュ上限: 5GB)
    let max_cache_size_bytes = 100 * 1024 * 1024 * 1024; // 5 GB
    let scheduler = Scheduler::new(cities_to_process, 2, max_cache_size_bytes);

    // 3. 並列ダウンロードおよび展開・処理の開始
    tracing::info!("並列ダウンロードおよび展開・処理を開始します...");
    let result = scheduler
        .run(|city, extracted_path, mp| async move {
            tracing::info!("都市: {} ({}) の処理を開始します", city.city, city.id);
            let pref_city = format!("{}{}", city.pref, city.city);

            // udxディレクトリを探索
            let udx_dir = match find_udx_dir(&extracted_path) {
                Some(path) => path,
                None => {
                    tracing::warn!(
                        "udxディレクトリが見つかりませんでした: {:?}",
                        extracted_path
                    );
                    return Ok(());
                }
            };

            // bldg と tran のフォルダパスを取得
            let bldg_dir = udx_dir.join("bldg");
            let tran_dir = udx_dir.join("tran");

            let mp_bldg = mp.clone();
            let mp_tran = mp;

            // FeatureType 別の処理を tokio::try_join! で非同期並列実行 (スレッド生成なし)
            let bldg_fut = async {
                if bldg_dir.is_dir() {
                    bldg::process_directory(bldg_dir, &pref_city, mp_bldg).await
                } else {
                    Ok(())
                }
            };

            let tran_fut = async {
                if tran_dir.is_dir() {
                    tran::process_directory(tran_dir, &pref_city, mp_tran).await
                } else {
                    Ok(())
                }
            };

            tokio::try_join!(bldg_fut, tran_fut)?;

            tracing::info!("都市: {} ({}) の処理が終了しました", city.city, city.id);
            Ok(())
        })
        .await;

    match result {
        Ok(_) => tracing::info!("すべての都市の処理が完了しました！"),
        Err(e) => tracing::error!("実行中にエラーが発生しました: {}", e),
    }
}

/// 展開フォルダ内から udx ディレクトリを探します (ネスト対応)
fn find_udx_dir(root: &Path) -> Option<PathBuf> {
    if root.join("udx").is_dir() {
        return Some(root.join("udx"));
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                let path = entry.path().join("udx");
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    None
}
