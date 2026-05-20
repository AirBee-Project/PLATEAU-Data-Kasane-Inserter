use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use crate::list::CityGml;
use crate::error::AppError;

// デフォルトのキャッシュ設定の定数定義
const CACHE_DIR: &str = "cache/download_cache";
const MAX_CACHE_SIZE_BYTES: u64 = 5 * 1024 * 1024 * 1024; // デフォルト上限: 5GB

/// 並列ダウンロード・処理スケジューラ
pub struct Scheduler {
    cities: Vec<CityGml>,
    concurrency_limit: usize,
}

impl Scheduler {
    /// 都市データと並列実行数を指定してスケジューラを初期化します。
    pub fn new(
        cities: Vec<CityGml>,
        concurrency_limit: usize,
    ) -> Self {
        Self {
            cities,
            concurrency_limit,
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

        for city in &self.cities {
            let permit = semaphore.clone().acquire_owned().await.map_err(|e| AppError::Other(e.to_string()))?;
            let handler = handler.clone();
            let city = city.clone();
            let mp = mp.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit; // タスク完了までパーミットを保持する
                let _ = mp.println(format!("都市の処理を開始します: {} ({})", city.city, city.id));
                let res = process_city(&city, mp.clone(), handler).await;
                if let Err(ref e) = res {
                    let _ = mp.println(format!("都市 {} ({}) の処理中にエラーが発生しました: {}", city.city, city.id, e));
                } else {
                    let _ = mp.println(format!("都市の処理が完了しました: {} ({})", city.city, city.id));
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
    mp: Arc<indicatif::MultiProgress>,
    handler: F,
) -> Result<(), AppError>
where
    F: Fn(CityGml, PathBuf) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), AppError>> + Send + 'static,
{
    // 1. ZIPファイルの取得 (キャッシュ)
    let zip_path = get_zip_file(city, mp.clone()).await?;

    // 2. 展開先フォルダのパス設定とチェック
    let extract_path = PathBuf::from("cache/temp_extracted").join(format!("city_{}", city.id));
    let is_extracted = extract_path.exists();

    if !is_extracted {
        fs::create_dir_all(&extract_path)?;

        // 3. ZIPファイルを展開 (ブロック処理のため spawn_blocking を使用)
        let zip_path_clone = zip_path.clone();
        let extract_path_clone = extract_path.clone();
        let mp_clone = mp.clone();
        let city_name = city.city.clone();
        tokio::task::spawn_blocking(move || {
            let _ = mp_clone.println(format!("ZIPファイルを展開中: {}...", city_name));
            extract_zip(&zip_path_clone, &extract_path_clone)
        })
        .await??;
    } else {
        let _ = mp.println(format!("展開キャッシュヒット: {} (フォルダ: {:?})", city.city, extract_path));
    }

    // 4. ユーザー定義の処理を実行
    let run_result = handler(city.clone(), extract_path.clone()).await;

    // 5. 不要な一時ファイルのクリーンアップは行わない (展開結果をキャッシュとして残すため)

    run_result?;
    Ok(())
}

/// ZIPファイルパスを取得 (キャッシュヒット時はそれを使用、ミスの場合はダウンロード)
async fn get_zip_file(city: &CityGml, mp: Arc<indicatif::MultiProgress>) -> Result<PathBuf, AppError> {
    let filename = format!("{}-{}.zip", city.pref_code, city.id);
    let cache_dir = Path::new(CACHE_DIR);
    let cached_path = cache_dir.join(&filename);

    // キャッシュに存在する場合はそれを使用
    if cached_path.exists() {
        let _ = mp.println(format!("キャッシュヒット: {} (ファイル: {:?})", city.city, cached_path));
        if let Ok(file) = fs::OpenOptions::new().write(true).open(&cached_path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        return Ok(cached_path);
    }

    // キャッシュに存在しない場合は一時名でダウンロード後にリネーム
    let _ = mp.println(format!("キャッシュミス: {}。ダウンロードを開始します...", city.city));
    let temp_download_path = cache_dir.join(format!("{}.download", filename));
    
    fs::create_dir_all(cache_dir)?;

    download_url_to_file(&city.url, &temp_download_path, &filename, mp.clone()).await?;
    fs::rename(&temp_download_path, &cached_path)?;

    // キャッシュサイズが上限を超えていれば古い順にクリーンアップ
    evict_cache(mp)?;

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
fn extract_zip(
    zip_path: &Path,
    extract_path: &Path,
) -> Result<(), AppError> {
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
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
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

/// 古いキャッシュファイルを削除して制限サイズ以下に保ちます (LRUライク)
fn evict_cache(mp: Arc<indicatif::MultiProgress>) -> Result<(), AppError> {
    let cache_dir = Path::new(CACHE_DIR);
    let mut files = Vec::new();
    let mut total_size = 0u64;

    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "download") {
                continue;
            }
            let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let size = metadata.len();
            total_size += size;
            files.push((path, modified, size));
        }
    }

    if total_size > MAX_CACHE_SIZE_BYTES {
        let _ = mp.println(format!(
            "キャッシュ制限を超過しました。現在: {} bytes > 制限: {} bytes. 古いファイルを削除します...",
            total_size, MAX_CACHE_SIZE_BYTES
        ));
        files.sort_by_key(|f| f.1); // 更新日時順 (古い順)

        for (path, _, size) in files {
            if total_size <= MAX_CACHE_SIZE_BYTES {
                break;
            }

            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            // 例: "01-01205" から "01205" を抽出する
            let city_id = stem.split('-').nth(1).unwrap_or("");

            let _ = mp.println(format!("キャッシュファイルを削除します: {:?}", path));
            if let Err(e) = fs::remove_file(&path) {
                let _ = mp.println(format!("警告: キャッシュファイル ({:?}) の削除に失敗しました: {}", path, e));
            } else {
                total_size -= size;
                // 対応する展開フォルダも削除
                if !city_id.is_empty() {
                    let ext_dir = PathBuf::from("cache/temp_extracted").join(format!("city_{}", city_id));
                    if ext_dir.exists() {
                        let _ = mp.println(format!("対応する展開フォルダを削除します: {:?}", ext_dir));
                        let _ = fs::remove_dir_all(&ext_dir);
                    }
                }
            }
        }
    }

    Ok(())
}
