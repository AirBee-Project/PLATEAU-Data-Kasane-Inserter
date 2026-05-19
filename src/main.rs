use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://api.plateauview.mlit.go.jp/datacatalog/plateau-datasets";

    // HTTP GET
    let response = reqwest::get(url).await?;

    // バイト列取得
    let bytes = response.bytes().await?;

    // ファイル保存
    let mut file = File::create("file.json").await?;
    file.write_all(&bytes).await?;

    println!("download complete");

    Ok(())
}
