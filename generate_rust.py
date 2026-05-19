import json

# Load parsed data
with open("parsed_data.json", "r", encoding="utf-8") as f:
    regions = json.load(f)

# Build Rust code
rust_lines = []

# Write types first
rust_lines.append("/// PLATEAUのデータセット情報")
rust_lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
rust_lines.append("pub struct Dataset {")
rust_lines.append("    /// 整備年度（西暦）")
rust_lines.append("    pub year: u16,")
rust_lines.append("    /// G空間情報センターのデータセットURL")
rust_lines.append("    pub url: &'static str,")
rust_lines.append("    /// タイトル")
rust_lines.append("    pub title: &'static str,")
rust_lines.append("    /// 特記事項（例：「南大沢のみ」等）")
rust_lines.append("    pub note: Option<&'static str>,")
rust_lines.append("}")
rust_lines.append("")

rust_lines.append("/// 都市・地方公共団体ごとのデータ")
rust_lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
rust_lines.append("pub struct City {")
rust_lines.append("    /// 都市名")
rust_lines.append("    pub name: &'static str,")
rust_lines.append("    /// 年度別のデータセット一覧")
rust_lines.append("    pub datasets: &'static [Dataset],")
rust_lines.append("}")
rust_lines.append("")

rust_lines.append("/// 都道府県ごとのデータ")
rust_lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
rust_lines.append("pub struct Prefecture {")
rust_lines.append("    /// 都道府県名")
rust_lines.append("    pub name: &'static str,")
rust_lines.append("    /// 都道府県内の都市一覧")
rust_lines.append("    pub cities: &'static [City],")
rust_lines.append("}")
rust_lines.append("")

rust_lines.append("/// 地方・エリアごとのデータ")
rust_lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
rust_lines.append("pub struct Region {")
rust_lines.append("    /// 地方名（例：「北海道」、「東北」、「関東」等）")
rust_lines.append("    pub name: &'static str,")
rust_lines.append("    /// 地方内の都道府県一覧")
rust_lines.append("    pub prefectures: &'static [Prefecture],")
rust_lines.append("}")
rust_lines.append("")

rust_lines.append("/// 日本全国のPLATEAU 3D都市モデルデータリスト（静的定義）")
rust_lines.append("pub const REGIONS: &[Region] = &[")

for region in regions:
    rust_lines.append(f"    Region {{")
    rust_lines.append(f'        name: "{region["name"]}",')
    rust_lines.append(f"        prefectures: &[")
    
    for pref in region["prefectures"]:
        rust_lines.append(f"            Prefecture {{")
        rust_lines.append(f'                name: "{pref["name"]}",')
        rust_lines.append(f"                cities: &[")
        
        for city in pref["cities"]:
            rust_lines.append(f"                    City {{")
            rust_lines.append(f'                        name: "{city["name"]}",')
            rust_lines.append(f"                        datasets: &[")
            
            for ds in city["datasets"]:
                note_str = f'Some("{ds["note"]}")' if ds["note"] else "None"
                rust_lines.append(f"                            Dataset {{")
                rust_lines.append(f'                                year: {ds["year"]},')
                rust_lines.append(f'                                url: "{ds["url"]}",')
                rust_lines.append(f'                                title: "{ds["title"]}",')
                rust_lines.append(f'                                note: {note_str},')
                rust_lines.append(f"                            }},")
                
            rust_lines.append(f"                        ],")
            rust_lines.append(f"                    }},")
            
        rust_lines.append(f"                ],")
        rust_lines.append(f"            }},")
        
    rust_lines.append(f"        ],")
    rust_lines.append(f"    }},")

rust_lines.append("];")
rust_lines.append("")

rust_lines.append("/// 全国分のPLATEAUのデータリストを取得する関数")
rust_lines.append("pub fn list() -> &'static [Region] {")
rust_lines.append("    REGIONS")
rust_lines.append("}")

# Write to list.rs
with open("src/list.rs", "w", encoding="utf-8") as f:
    f.write("\n".join(rust_lines) + "\n")

print("Generated src/list.rs successfully!")
