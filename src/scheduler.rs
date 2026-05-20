use crate::error::AppError;
use crate::list::CityGml;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info, warn, error};

// デフォルトのキャッシュフォルダ定義
const CACHE_DIR: &str = "cache/download_cache";

/// 並列ダウンロード・処理スケジューラ
pub struct Scheduler {
    cities: Vec<CityGml>,
    concurrency_limit: usize,
    max_cache_size_bytes: u64,
}

impl Scheduler {
    /// 都市データ、並列実行数、キャッシュ容量上限（バイト）を指定してスケジューラを初期化します。
    pub fn new(cities: Vec<CityGml>, concurrency_limit: usize, max_cache_size_bytes: u64) -> Self {
        Self {
            cities,
            concurrency_limit,
            max_cache_size_bytes,
        }
    }

    /// 各都市のデータを並列でダウンロード・展開し、ユーザー提供のハンドラ関数を呼び出します。
    pub async fn run<F, Fut>(&self, handler: F) -> Result<(), AppError>
    where
        F: Fn(CityGml, PathBuf) -> Fut + Clone + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), AppError>> + Send + 'static,
    {
        let semaphore = Arc::new(Semaphore::new(self.concurrency_limit));
        let mp = Arc::new(indicatif::MultiProgress::new());
        let mut futures = Vec::new();
        let max_cache_size = self.max_cache_size_bytes;

        for city in &self.cities {
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| AppError::Other(e.to_string()))?;
            let handler = handler.clone();
            let city = city.clone();
            let mp = mp.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit; // タスク完了までパーミットを保持する
                info!("都市の処理を開始します: {} ({})", city.city, city.id);
                
                let res = process_city(&city, max_cache_size, mp.clone(), handler).await;
                if let Err(ref e) = res {
                    error!("都市 {} ({}) の処理中にエラーが発生しました: {}", city.city, city.id, e);
                } else {
                    info!("都市の処理が完了しました: {} ({})", city.city, city.id);
                }
                res
            });
            futures.push(handle);
        }

        // すべての処理タスクの完了を待機する
        for f in futures {
            let _ = f.await;
        }

        Ok(())
    }
}

/// 個別の都市データを処理する内部フロー
async fn process_city<F, Fut>(
    city: &CityGml,
    max_cache_size_bytes: u64,
    mp: Arc<indicatif::MultiProgress>,
    handler: F,
) -> Result<(), AppError>
where
    F: Fn(CityGml, PathBuf) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), AppError>> + Send + 'static,
{
    // 1. ZIPファイルの取得 (キャッシュ)
    let zip_path = get_zip_file(city, max_cache_size_bytes, mp.clone()).await?;

    // 2. 展開先フォルダのパス設定とチェック
    let extract_path = PathBuf::from("cache/temp_extracted").join(format!("city_{}", city.id));
    let is_extracted = extract_path.exists();

    if !is_extracted {
        fs::create_dir_all(&extract_path)?;

        // 3. ZIPファイルを展開 (ブロック処理のため spawn_blocking を使用)
        let zip_path_clone = zip_path.clone();
        let extract_path_clone = extract_path.clone();
        let city_name = city.city.clone();
        tokio::task::spawn_blocking(move || {
            info!("ZIPファイルを展開中: {}...", city_name);
            extract_zip(&zip_path_clone, &extract_path_clone)
        })
        .await??;
    } else {
        info!("展開キャッシュヒット: {} (フォルダ: {:?})", city.city, extract_path);
    }

    // 4. ユーザー定義の処理を実行
    let run_result = handler(city.clone(), extract_path.clone()).await;

    // 5. 不要な一時ファイルのクリーンアップは行わない (展開結果をキャッシュとして残すため)

    run_result?;
    Ok(())
}

/// ZIPファイルパスを取得 (キャッシュヒット時はそれを使用、ミスの場合はダウンロード)
async fn get_zip_file(
    city: &CityGml,
    max_cache_size_bytes: u64,
    mp: Arc<indicatif::MultiProgress>,
) -> Result<PathBuf, AppError> {
    let filename = format!("{}-{}.zip", city.pref_code, city.id);
    let cache_dir = Path::new(CACHE_DIR);
    let cached_path = cache_dir.join(&filename);

    // キャッシュに存在する場合はそれを使用
    if cached_path.exists() {
        info!("キャッシュヒット: {} (ファイル: {:?})", city.city, cached_path);
        if let Ok(file) = fs::OpenOptions::new().write(true).open(&cached_path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        return Ok(cached_path);
    }

    // キャッシュに存在しない場合は一時名でダウンロード後にリネーム
    info!("キャッシュミス: {}。ダウンロードを開始します...", city.city);
    let temp_download_path = cache_dir.join(format!("{}.download", filename));

    fs::create_dir_all(cache_dir)?;

    download_url_to_file(&city.url, &temp_download_path, &filename, mp.clone()).await?;
    fs::rename(&temp_download_path, &cached_path)?;

    // キャッシュサイズが上限を超えていれば古い順にクリーンアップ
    evict_cache(max_cache_size_bytes)?;

    Ok(cached_path)
}

/// 指定したURLから非同期ストリーミングでファイルへ保存します (メモリ逼迫を防ぐため)
async fn download_url_to_file(
    url: &str,
    path: &Path,
    filename: &str,
    mp: Arc<indicatif::MultiProgress>,
) -> Result<(), AppError> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(AppError::HttpStatus(response.status()));
    }

    let content_length = response.content_length();
    let pb = if let Some(len) = content_length {
        mp.add(indicatif::ProgressBar::new(len))
    } else {
        mp.add(indicatif::ProgressBar::new_spinner())
    };

    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) - {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(filename.to_string());

    let mut file = tokio::fs::File::create(path).await?;
    let mut response = response;
    while let Some(chunk) = response.chunk().await? {
        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message(format!("{} (ダウンロード完了)", filename));
    Ok(())
}

/// ZIPファイルを指定フォルダに展開します
fn extract_zip(zip_path: &Path, extract_path: &Path) -> Result<(), AppError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => extract_path.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent().filter(|p| !p.exists()) {
                fs::create_dir_all(p)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }
    }

    Ok(())
}

/// ディレクトリサイズを再帰的に計算します
fn dir_size(path: &Path) -> io::Result<u64> {
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    let mut size = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            size += entry.metadata()?.len();
        } else if entry_path.is_dir() {
            size += dir_size(&entry_path)?;
        }
    }
    Ok(size)
}

/// 古いキャッシュファイルを削除して制限サイズ以下に保ちます (LRUライク)
/// ZIPファイルと展開先フォルダの両方の容量を含めて判定し、削除時は両方を削除します。
fn evict_cache(max_cache_size_bytes: u64) -> Result<(), AppError> {
    let cache_dir = Path::new(CACHE_DIR);
    if !cache_dir.exists() {
        return Ok(());
    }

    struct CacheItem {
        zip_path: PathBuf,
        ext_dir: PathBuf,
        modified: std::time::SystemTime,
        size: u64,
    }

    let mut items = Vec::new();
    let mut total_size = 0u64;

    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "download") {
                continue;
            }
            let modified = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let zip_size = metadata.len();

            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            // 例: "01-01205" から "01205" を抽出する
            let city_id = stem.split('-').nth(1).unwrap_or("");

            // 対応する展開フォルダの容量も合算する
            let mut ext_size = 0;
            let ext_dir = PathBuf::from("cache/temp_extracted").join(format!("city_{}", city_id));
            if ext_dir.exists() {
                ext_size = dir_size(&ext_dir).unwrap_or(0);
            }

            let item_total_size = zip_size + ext_size;
            total_size += item_total_size;

            items.push(CacheItem {
                zip_path: path,
                ext_dir,
                modified,
                size: item_total_size,
            });
        }
    }

    if total_size > max_cache_size_bytes {
        info!(
            "キャッシュ制限を超過しました。現在: {} bytes > 制限: {} bytes. 古いファイルを削除します...",
            total_size, max_cache_size_bytes
        );
        items.sort_by_key(|item| item.modified); // 更新日時順 (古い順)

        for item in items {
            if total_size <= max_cache_size_bytes {
                break;
            }

            info!(
                "キャッシュを削除します: ZIP: {:?}, 展開フォルダ: {:?}",
                item.zip_path, item.ext_dir
            );

            // ZIPファイルを削除
            if let Err(e) = fs::remove_file(&item.zip_path) {
                warn!(
                    "警告: ZIPファイル ({:?}) の削除に失敗しました: {}",
                    item.zip_path, e
                );
            } else {
                total_size -= item.size;

                // 対応する展開フォルダも削除
                if item.ext_dir.exists() {
                    let _ = fs::remove_dir_all(&item.ext_dir).map_err(|e| {
                        warn!(
                            "警告: 展開フォルダ ({:?}) の削除に失敗しました: {}",
                            item.ext_dir, e
                        );
                    });
                }
            }
        }
    }

    Ok(())
}
