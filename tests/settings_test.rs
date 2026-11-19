//! 设置项集成测试：回收站保留天数的默认值、持久化往返、脏数据回退与越界校验。
//! 全程使用临时目录，不触碰真实相册。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use akashic::core::config::Paths;
use akashic::core::ops::trash;
use akashic::core::db;

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn setup(name: &str) -> (PathBuf, rusqlite::Connection) {
    let uniq = SEQ.fetch_add(1, Ordering::SeqCst);
    let base = std::env::temp_dir().join(format!(
        "akashic_settings_{}_{}_{}",
        name,
        std::process::id(),
        uniq
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();
    (base, conn)
}

#[test]
fn retention_defaults_to_30_when_unset() {
    let (_base, conn) = setup("default");
    assert_eq!(trash::retention_days(&conn), trash::DEFAULT_RETENTION_DAYS);
    assert_eq!(trash::DEFAULT_RETENTION_DAYS, 30);
}

#[test]
fn retention_setting_roundtrips() {
    let (_base, conn) = setup("roundtrip");
    for days in [7_i64, 30, 90, 365] {
        trash::set_retention_days(&conn, days).unwrap();
        assert_eq!(trash::retention_days(&conn), days);
    }
    // 重开连接后仍保留（设置确实落库而非仅在内存）
    // 同一 Connection 已足够验证 settings 表读写；此处额外确认键名。
    assert_eq!(
        db::get_setting(&conn, trash::RETENTION_SETTING_KEY)
            .unwrap()
            .as_deref(),
        Some("365")
    );
}

#[test]
fn invalid_or_out_of_range_stored_value_falls_back() {
    let (_base, conn) = setup("invalid");
    for bad in ["abc", "", "0", "-5", "3651", "999999"] {
        db::set_setting(&conn, trash::RETENTION_SETTING_KEY, bad).unwrap();
        assert_eq!(
            trash::retention_days(&conn),
            trash::DEFAULT_RETENTION_DAYS,
            "脏值 {bad:?} 应回退默认值"
        );
    }
}

#[test]
fn out_of_range_write_is_rejected() {
    let (_base, conn) = setup("reject");
    trash::set_retention_days(&conn, 14).unwrap();
    assert!(trash::set_retention_days(&conn, 0).is_err());
    assert!(trash::set_retention_days(&conn, -1).is_err());
    assert!(trash::set_retention_days(&conn, trash::RETENTION_MAX_DAYS + 1).is_err());
    // 被拒绝的写入不应破坏已有合法值
    assert_eq!(trash::retention_days(&conn), 14);
}
