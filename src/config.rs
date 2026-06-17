use crate::error::AppError;
use crate::kasane::models::ZoomLevelPolicy;
use std::env;

/// Kasane への挿入に関する設定。すべて環境変数から構築します。
///
/// 役割ごとに「秘匿情報」「接続/投入先」「変換・挿入の挙動」に分けています。
pub struct KasaneConfig {
    // ── 秘匿情報（ログ出力禁止）──
    /// ログインユーザー名 (`KASANE_USERNAME`)
    pub username: String,
    /// ログインパスワード (`KASANE_PASSWORD`)
    pub password: String,

    // ── 接続/投入先 ──
    /// API のベース URL (`KASANE_BASE_URL`、既定 `http://localhost:3000`)
    pub base_url: String,
    /// 投入先データベース名 (`KASANE_DATABASE`、既定 `plateau`)
    pub database: String,
    /// 建築物の投入先テーブル名 (`KASANE_BLDG_TABLE`、既定 `bldg`)
    pub bldg_table: String,
    /// 交通の投入先テーブル名 (`KASANE_TRAN_TABLE`、既定 `tran`)
    pub tran_table: String,

    // ── 変換・挿入の挙動 ──
    /// 変換時のズームレベル (`KASANE_MAX_ZOOM`、既定 25)。
    /// 投入先テーブルの `max_zoom_level` と一致させる必要があります。
    pub max_zoom: u8,
    /// 挿入時のズームレベルポリシー (`KASANE_ZOOM_POLICY`、既定 `Normalize`)
    pub zoom_policy: ZoomLevelPolicy,

    // ── 性能チューニング ──
    /// 1 リクエストあたりの空間 ID 数 (`KASANE_INSERT_BATCH`、既定 5000)
    pub insert_batch: usize,
    /// 挿入リクエストの並列度 (`KASANE_INSERT_CONCURRENCY`、既定 4)
    pub insert_concurrency: usize,

    // ── デバッグ ──
    /// 各 GML ファイルあたり先頭 N 件の空間 ID だけを挿入する制限
    /// (`KASANE_DEBUG_LIMIT`、未設定なら無制限)。
    pub debug_limit_per_file: Option<usize>,
}

impl KasaneConfig {
    /// 環境変数から設定を読み込みます。認証情報が無い場合はエラーになります。
    pub fn from_env() -> Result<Self, AppError> {
        let username = env::var("KASANE_USERNAME")
            .map_err(|_| AppError::Other("環境変数 KASANE_USERNAME が未設定です".into()))?;
        let password = env::var("KASANE_PASSWORD")
            .map_err(|_| AppError::Other("環境変数 KASANE_PASSWORD が未設定です".into()))?;

        let base_url =
            env::var("KASANE_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into());
        let database = env::var("KASANE_DATABASE").unwrap_or_else(|_| "plateau".into());
        let bldg_table = env::var("KASANE_BLDG_TABLE").unwrap_or_else(|_| "bldg".into());
        let tran_table = env::var("KASANE_TRAN_TABLE").unwrap_or_else(|_| "tran".into());

        let max_zoom = env::var("KASANE_MAX_ZOOM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25);

        let zoom_policy = env::var("KASANE_ZOOM_POLICY")
            .ok()
            .and_then(|v| ZoomLevelPolicy::parse(&v))
            .unwrap_or(ZoomLevelPolicy::Normalize);

        let insert_batch = env::var("KASANE_INSERT_BATCH")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(5000);

        let insert_concurrency = env::var("KASANE_INSERT_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(4);

        let debug_limit_per_file = env::var("KASANE_DEBUG_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok());

        Ok(Self {
            username,
            password,
            base_url,
            database,
            bldg_table,
            tran_table,
            max_zoom,
            zoom_policy,
            insert_batch,
            insert_concurrency,
            debug_limit_per_file,
        })
    }
}
