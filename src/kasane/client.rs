use crate::config::KasaneConfig;
use crate::error::AppError;
use crate::kasane::models::{
    InsertDataRequest, LoginRequest, LoginResponse, SpatialId, ZoomLevelPolicy,
};
use reqwest::StatusCode;
use tokio::sync::RwLock;

/// Kasane データベース API の HTTP クライアント。
///
/// JWT トークンを内部に保持し、401 を検知した際は自動で再ログインして 1 回だけ再試行します。
pub struct KasaneClient {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
    token: RwLock<String>,
}

impl KasaneClient {
    /// 設定をもとにクライアントを生成し、ログインまで済ませて返します。
    pub async fn connect(cfg: &KasaneConfig) -> Result<Self, AppError> {
        let client = Self {
            http: reqwest::Client::new(),
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            token: RwLock::new(String::new()),
        };
        client.login().await?;
        tracing::info!("Kasane へのログインに成功しました: {}", client.base_url);
        Ok(client)
    }

    /// 認証情報でログインし、取得したトークンを内部に保存します。
    async fn login(&self) -> Result<(), AppError> {
        let url = format!("{}/auth/login", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&LoginRequest {
                username: &self.username,
                password: &self.password,
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::Auth);
        }

        let body: LoginResponse = resp.json().await?;
        *self.token.write().await = body.token;
        Ok(())
    }

    /// 指定したテーブルへ空間 ID 群に同一の値を挿入します（`PUT data_insert`）。
    ///
    /// トークン失効（401）時は一度だけ再ログインして再試行します。
    pub async fn insert_data(
        &self,
        db: &str,
        table: &str,
        value: serde_json::Value,
        spatial_ids: &[SpatialId],
        zoom_level_policy: ZoomLevelPolicy,
    ) -> Result<(), AppError> {
        if spatial_ids.is_empty() {
            return Ok(());
        }

        let url = format!("{}/databases/{}/tables/{}/data", self.base_url, db, table);
        let req = InsertDataRequest {
            value,
            spatial_ids,
            zoom_level_policy,
        };

        for attempt in 0..2 {
            let token = self.token.read().await.clone();
            let resp = self
                .http
                .put(&url)
                .bearer_auth(&token)
                .json(&req)
                .send()
                .await?;

            let status = resp.status();
            if status.is_success() {
                return Ok(());
            }

            if status == StatusCode::UNAUTHORIZED && attempt == 0 {
                tracing::warn!("認証トークンが失効しました。再ログインして再試行します...");
                self.login().await?;
                continue;
            }

            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::KasaneApi { status, body });
        }

        // ループは必ず return で抜けるためここには到達しない
        unreachable!()
    }
}
