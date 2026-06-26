use kasane_logic::{RangeId, SingleId};
use serde::{Deserialize, Serialize};

/// `POST /auth/login` のリクエストボディ
#[derive(Serialize)]
pub struct LoginRequest<'a> {
    pub username: &'a str,
    pub password: &'a str,
}

/// `POST /auth/login` のレスポンスボディ
#[derive(Deserialize)]
pub struct LoginResponse {
    pub token: String,
}

/// 各 Table の `max_zoom_level` より小さな ID が入力された場合の挙動。
///
/// API の `ZoomLevelPolicy` 列挙に対応し、variant 名がそのまま JSON 文字列になります。
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomLevelPolicy {
    Error,
    Ignore,
    Normalize,
}

impl ZoomLevelPolicy {
    /// 環境変数の文字列から大文字小文字を無視してパースします。
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Some(Self::Error),
            "ignore" => Some(Self::Ignore),
            "normalize" => Some(Self::Normalize),
            _ => None,
        }
    }
}

/// API の `SpatialId`(`singleId` / `rangeId` の内部タグ付き表現)。
///
/// `kasane_logic` の ID 型はフィールドが private かつ `Serialize` 非対応のため、
/// 公開アクセサ経由でこの型に変換してから JSON 化します。
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum SpatialId {
    #[serde(rename = "singleId")]
    Single { z: u8, f: i32, x: u32, y: u32 },
    #[serde(rename = "rangeId")]
    Range {
        z: u8,
        f: [i32; 2],
        x: [u32; 2],
        y: [u32; 2],
    },
}

impl From<&SingleId> for SpatialId {
    fn from(id: &SingleId) -> Self {
        SpatialId::Single {
            z: id.z(),
            f: id.f(),
            x: id.x(),
            y: id.y(),
        }
    }
}

impl From<&RangeId> for SpatialId {
    fn from(id: &RangeId) -> Self {
        SpatialId::Range {
            z: id.z(),
            f: id.f(),
            x: id.x(),
            y: id.y(),
        }
    }
}

/// `PUT /databases/{db}/tables/{table}/data`(`data_insert`)のリクエストボディ。
#[derive(Serialize)]
pub struct InsertDataRequest<'a> {
    pub value: serde_json::Value,
    pub spatial_ids: &'a [SpatialId],
    pub zoom_level_policy: ZoomLevelPolicy,
}
