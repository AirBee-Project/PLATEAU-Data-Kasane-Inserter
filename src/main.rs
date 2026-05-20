use std::path::{Path, PathBuf};
use plateau_data_kasane_inserter::list::CityList;
use plateau_data_kasane_inserter::scheduler::Scheduler;
use plateau_data_kasane_inserter::features::{bldg, tran};

#[tokio::main]
async fn main() {
    let url = "https://api.plateauview.mlit.go.jp/datacatalog/plateau-datasets";

    // 1. 都市の一覧を取得 (デフォルトで "cache/plateau_cache.json" をキャッシュに使用)
    let city_list = match CityList::new(url).await {
        Ok(list) => list,
        Err(e) => {
            eprintln!("都市一覧の取得に失敗しました: {}", e);
            return;
        }
    };

    // 件数制限を設定 (デバッグが容易になるよう、最初の3件のみに制限)
    let city_list = city_list.take(3);
    let cities_to_process = city_list.cities().to_vec();
    println!("処理対象の都市数: {}", cities_to_process.len());

    // 2. スケジューラの初期化 (並列実行数: 2, デフォルトのキャッシュ設定 "cache/download_cache" 上限5GBを使用)
    let scheduler = Scheduler::new(cities_to_process, 2);

    // 3. 並列ダウンロードおよび展開・処理の開始
    println!("並列ダウンロードおよび展開・処理を開始します...");
    let result = scheduler.run(|city, extracted_path| async move {
        println!(
            "\n[USER PROCESS] >>> 都市: {} ({}) の処理を開始します。",
            city.city, city.id
        );

        // udxディレクトリを探索
        let udx_dir = match find_udx_dir(&extracted_path) {
            Some(path) => path,
            None => {
                eprintln!("[USER PROCESS] udxディレクトリが見つかりませんでした: {:?}", extracted_path);
                return Ok(());
            }
        };

        // bldg と tran のフォルダパスを取得
        let bldg_dir = udx_dir.join("bldg");
        let tran_dir = udx_dir.join("tran");

        // FeatureType 別の処理を別スレッドかつ並列で実行
        let bldg_handle = tokio::spawn(async move {
            if bldg_dir.is_dir() {
                if let Err(e) = bldg::process_directory(bldg_dir).await {
                    eprintln!("bldgディレクトリの処理エラー: {}", e);
                }
            }
        });

        let tran_handle = tokio::spawn(async move {
            if tran_dir.is_dir() {
                if let Err(e) = tran::process_directory(tran_dir).await {
                    eprintln!("tranディレクトリの処理エラー: {}", e);
                }
            }
        });

        let _ = tokio::join!(bldg_handle, tran_handle);

        println!("[USER PROCESS] >>> 都市: {} ({}) の処理が終了しました。\n", city.city, city.id);
        Ok(())
    }).await;

    match result {
        Ok(_) => println!("すべての都市の処理が完了しました！"),
        Err(e) => eprintln!("実行中にエラーが発生しました: {}", e),
    }
}

/// 展開フォルダ内から udx ディレクトリを探します (ネスト対応)
fn find_udx_dir(root: &Path) -> Option<PathBuf> {
    if root.join("udx").is_dir() {
        return Some(root.join("udx"));
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                let path = entry.path().join("udx");
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    None
}
