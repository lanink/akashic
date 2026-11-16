//! 数据模型骨架（M0 阶段沿用设计稿模拟数据，行为与原型一致）。
//! M1 将用真实索引数据库替换 `PHOTOS` 与 `folder_defs()`。

/// 设计稿中的模拟照片定义。
pub struct PhotoDef {
    pub name: &'static str,
    pub date: &'static str,
    pub size: &'static str,
    pub file_size: &'static str,
    pub device: &'static str,
    pub location: &'static str,
    pub tags: &'static [&'static str],
    /// 缩略图色相（0..360）
    pub hue: f32,
}

pub const PHOTOS: &[PhotoDef] = &[
    PhotoDef { name: "IMG_20240815_094523.jpg", date: "2024-08-15 09:45", size: "4032×3024", file_size: "4.2 MB", device: "iPhone 15 Pro", location: "日本 · 富士山", tags: &["旅行", "风景", "富士山", "2024"], hue: 8.0 },
    PhotoDef { name: "IMG_20240815_102134.jpg", date: "2024-08-15 10:21", size: "4032×3024", file_size: "3.8 MB", device: "iPhone 15 Pro", location: "日本 · 河口湖", tags: &["旅行", "风景", "河口湖"], hue: 200.0 },
    PhotoDef { name: "IMG_20240815_143022.jpg", date: "2024-08-15 14:30", size: "3024×4032", file_size: "5.1 MB", device: "iPhone 15 Pro", location: "日本 · 浅草寺", tags: &["旅行", "寺庙", "浅草"], hue: 338.0 },
    PhotoDef { name: "IMG_20240816_082145.jpg", date: "2024-08-16 08:21", size: "4032×3024", file_size: "6.3 MB", device: "iPhone 15 Pro", location: "日本 · 新干线", tags: &["旅行", "列车", "新干线"], hue: 255.0 },
    PhotoDef { name: "IMG_20240816_120033.jpg", date: "2024-08-16 12:00", size: "4032×3024", file_size: "4.7 MB", device: "iPhone 15 Pro", location: "日本 · 京都", tags: &["旅行", "美食", "京都"], hue: 40.0 },
    PhotoDef { name: "IMG_20240816_163845.jpg", date: "2024-08-16 16:38", size: "3024×4032", file_size: "3.2 MB", device: "iPhone 15 Pro", location: "日本 · 伏见稻荷", tags: &["旅行", "神社", "鸟居"], hue: 350.0 },
    PhotoDef { name: "IMG_20240817_091234.jpg", date: "2024-08-17 09:12", size: "4032×3024", file_size: "5.8 MB", device: "iPhone 15 Pro", location: "日本 · 奈良", tags: &["旅行", "鹿", "奈良"], hue: 150.0 },
    PhotoDef { name: "IMG_20240817_141522.jpg", date: "2024-08-17 14:15", size: "4032×3024", file_size: "4.4 MB", device: "iPhone 15 Pro", location: "日本 · 大阪", tags: &["旅行", "城市", "大阪"], hue: 280.0 },
    PhotoDef { name: "IMG_20240818_103045.jpg", date: "2024-08-18 10:30", size: "3024×4032", file_size: "3.9 MB", device: "iPhone 15 Pro", location: "日本 · 东京塔", tags: &["旅行", "地标", "东京"], hue: 60.0 },
    PhotoDef { name: "IMG_20240818_193022.jpg", date: "2024-08-18 19:30", size: "4032×3024", file_size: "2.8 MB", device: "iPhone 15 Pro", location: "日本 · 涩谷", tags: &["旅行", "夜景", "涩谷"], hue: 230.0 },
    PhotoDef { name: "IMG_20240819_075634.jpg", date: "2024-08-19 07:56", size: "4032×3024", file_size: "6.1 MB", device: "iPhone 15 Pro", location: "日本 · 镰仓", tags: &["旅行", "海岸", "镰仓"], hue: 190.0 },
    PhotoDef { name: "IMG_20240819_123045.jpg", date: "2024-08-19 12:30", size: "4032×3024", file_size: "3.5 MB", device: "iPhone 15 Pro", location: "日本 · 江之岛", tags: &["旅行", "海岛", "江之岛"], hue: 122.0 },
];

/// 相册目录树节点。
pub struct FolderNode {
    pub name: &'static str,
    pub count: usize,
    pub children: Vec<FolderNode>,
}

/// 构造静态相册目录树（M1 将从真实路径派生）。
pub fn folder_defs() -> Vec<FolderNode> {
    vec![
        FolderNode { name: "日本旅行", count: 85, children: vec![] },
        FolderNode { name: "生日派对", count: 34, children: vec![] },
        FolderNode { name: "四姑娘山", count: 47, children: vec![] },
        FolderNode {
            name: "2024年",
            count: 328,
            children: vec![
                FolderNode { name: "青岛旅行", count: 156, children: vec![] },
                FolderNode { name: "公司团建", count: 47, children: vec![] },
            ],
        },
        FolderNode {
            name: "2023年",
            count: 892,
            children: vec![
                FolderNode { name: "三亚旅行", count: 203, children: vec![] },
                FolderNode { name: "圣诞聚餐", count: 28, children: vec![] },
            ],
        },
    ]
}

/// 计算一个深度优先扁平索引是否为可展开节点；是则返回它在展开状态数组中的下标。
pub fn expandable_id(folders: &[FolderNode], target: usize) -> Option<usize> {
    let mut order = vec![];
    fn walk(folders: &[FolderNode], idx: &mut usize, order: &mut Vec<usize>) {
        for f in folders {
            if !f.children.is_empty() {
                order.push(*idx);
            }
            *idx += 1;
            walk(&f.children, idx, order);
        }
    }
    walk(folders, &mut 0, &mut order);
    order.iter().position(|&n| n == target)
}
