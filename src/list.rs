use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// JSONのルートオブジェクト
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CatalogRoot {
    pub latest_citygml: Vec<CityGml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FeatureType {
    Bldg, // 建築物
    Tran, // 交通（道路・鉄道）
    Luse, // 土地利用
    Fld,  // 洪水浸水想定区域
    Tnm,  // 津波浸水想定区域
    Lsld, // 土砂災害警戒区域
    Urf,  // 都市計画決定情報
    Ubld, // 地下街
    Dem,  // 数値標高モデル(地形)
    Frn,  // 都市設備
    Veg,  // 植生
    #[serde(other)]
    Unknown,
}

// 配列の中の各都市のデータ
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CityGml {
    pub id: String,
    pub pref: String,
    pub pref_code: String,
    pub city: String,
    pub city_code: String,
    pub url: String,
    pub file_size: u64,
    pub feature_types: Vec<FeatureType>,
    pub year: String,
}

const CATALOG_URL: &str = "https://api.plateauview.mlit.go.jp/datacatalog/plateau-datasets";

/// JSONデータをキャッシュし、都市データをイテレートする型
pub struct CityList {
    cities: Vec<CityGml>,
    cursor: usize,
}

impl CityList {
    /// キャッシュが存在すればそこから、なければURLからデータを取得してキャッシュを作成し、CityListを初期化します。
    /// キャッシュファイル名はデフォルトで "plateau_cache.json" になります。
    pub async fn new() -> Result<Self, crate::error::AppError> {
        Self::new_with_cache_path("cache/plateau_cache.json").await
    }

    /// カスタムキャッシュパスを指定してCityListを初期化します。
    pub async fn new_with_cache_path<P: AsRef<Path>>(
        cache_path: P,
    ) -> Result<Self, crate::error::AppError> {
        let cache_path = cache_path.as_ref();

        let bytes = if cache_path.exists() {
            fs::read(cache_path)?
        } else {
            let response = reqwest::get(CATALOG_URL).await?;
            let bytes = response.bytes().await?;
            if let Some(parent) = cache_path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            fs::write(cache_path, &bytes)?;
            bytes.to_vec()
        };

        let catalog: CatalogRoot = serde_json::from_slice(&bytes)?;
        Ok(Self {
            cities: catalog.latest_citygml,
            cursor: 0,
        })
    }

    /// イテレータのカーソル位置を最初に戻します。
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    /// 都市データの総数を取得します。
    pub fn len(&self) -> usize {
        self.cities.len()
    }

    /// 都市データが空かどうかを判定します。
    pub fn is_empty(&self) -> bool {
        self.cities.is_empty()
    }

    /// 内部のベクタへの参照を取得します。
    pub fn cities(&self) -> &[CityGml] {
        &self.cities
    }

    /// 件数制限を設定します (先頭から指定件数のみ残します)。
    pub fn take(mut self, limit: usize) -> Self {
        self.cities.truncate(limit);
        self
    }
}

// `CityList` 自体をイテレータとして扱えるようにする実装
impl Iterator for CityList {
    type Item = CityGml;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor < self.cities.len() {
            let item = self.cities[self.cursor].clone();
            self.cursor += 1;
            Some(item)
        } else {
            None
        }
    }
}
