use plateau_data_kasane_inserter::config::KasaneConfig;
use plateau_data_kasane_inserter::features::{bldg, tran};
use plateau_data_kasane_inserter::kasane::client::KasaneClient;
use plateau_data_kasane_inserter::list::CityList;
use plateau_data_kasane_inserter::scheduler::Scheduler;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // .env ファイルがあれば環境変数を読み込む（存在しなくてもエラーにしない）
    let _ = dotenvy::dotenv();

    // ログを初期化
    tracing_subscriber::fmt::init();

    // 都市データの一覧を取得
    let city_list = match CityList::new().await {
        Ok(list) => list,
        Err(e) => {
            tracing::error!("都市一覧の取得に失敗しました: {}", e);
            return;
        }
    };

    // 処理する都市データを環境変数から取得
    let city_limit = std::env::var("CITY_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let city_list = city_list.take(city_limit);
    let cities_to_process = city_list.cities().to_vec();
    tracing::info!("処理対象の都市数: {}", cities_to_process.len());

    // 都市の並列処理数を環境変数から取得
    let city_concurrency = std::env::var("CITY_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    // Kasane の設定を環境変数から構築
    let kasane_cfg = match KasaneConfig::from_env() {
        Ok(cfg) => Arc::new(cfg),
        Err(e) => {
            tracing::error!("Kasane 設定の読み込みに失敗しました: {}", e);
            return;
        }
    };

    // Kasane へログインしてクライアントを生成
    let kasane_client = match KasaneClient::connect(&kasane_cfg).await {
        Ok(client) => Arc::new(client),
        Err(e) => {
            tracing::error!("Kasane への接続に失敗しました: {}", e);
            return;
        }
    };

    // CityGMLのダウンロードと処理を実行
    let scheduler = Scheduler::new(
        cities_to_process,
        city_concurrency,
        100 * 1024 * 1024 * 1024,
    );

    // ダウンロードが完了した都市から処理を開始
    let result = scheduler
        .run(move |city, extracted_path, mp| {
            let client = kasane_client.clone();
            let cfg = kasane_cfg.clone();
            async move {
                tracing::info!("都市: {} ({}) の処理を開始", city.city, city.id);
                let pref_city = format!("{}{}", city.pref, city.city);

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

                let bldg_dir = udx_dir.join("bldg");
                let tran_dir = udx_dir.join("tran");

                let mp_bldg = mp.clone();
                let mp_tran = mp;

                let bldg_fut = async {
                    if bldg_dir.is_dir() {
                        bldg::process_directory(bldg_dir, &pref_city, mp_bldg, client.clone(), cfg.clone()).await
                    } else {
                        Ok(())
                    }
                };

                let tran_fut = async {
                    if tran_dir.is_dir() {
                        tran::process_directory(tran_dir, &pref_city, mp_tran, client.clone(), cfg.clone()).await
                    } else {
                        Ok(())
                    }
                };

                tokio::try_join!(bldg_fut, tran_fut)?;

                tracing::info!("都市: {} ({}) の処理が終了しました", city.city, city.id);
                Ok(())
            }
        })
        .await;

    match result {
        Ok(_) => tracing::info!("すべての都市の処理が完了しました！"),
        Err(e) => tracing::error!("実行中にエラーが発生しました: {}", e),
    }
}

/// 展開フォルダ内から udx ディレクトリを探す関数
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
