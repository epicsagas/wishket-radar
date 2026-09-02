//! 신뢰 경계: 대시보드가 읽고 쓸 수 있는 경로 검증 + 원자 쓰기.
//! 모든 파일 엔드포인트는 이 모듈의 `resolve`를 통해서만 실제 경로를 만든다.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub mtime_epoch: u64,
    /// 파일명에서 뽑은 위시켓 공고 ID (있으면). 제안서를 공고별로 묶는 데 쓴다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

/// 파일명에서 위시켓 공고 ID(5~6자리 숫자)를 찾는다.
/// "2026-09-02-158080-submit.md" → 158080, "158092-proposal.md" → 158092.
/// 날짜(2026, 09, 02)는 자릿수로 걸러진다.
pub fn project_id_from_name(name: &str) -> Option<String> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter(|t| (5..=7).contains(&t.len()))
        .map(String::from)
        .next()
}

/// 파일명이 루트 밖으로 나가지 않는지 검증하고 최종 경로를 반환한다.
/// 이름에 경로 구분자/상위 참조가 있으면 거부한다.
pub fn resolve(root: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains(':')
        || name.contains('\0')
    {
        return Err(format!("invalid file name: {name:?}"));
    }
    let joined = root.join(name);
    if let Some(parent) = joined.parent() {
        // 심볼릭 링크 등으로 루트가 가리키는 실제 위치와 일치하는지 확인
        let real_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let real_parent = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        if real_parent != real_root {
            return Err(format!("path escapes root: {name:?}"));
        }
    }
    Ok(joined)
}

/// 원자 쓰기: 부모 생성 → 기존 파일 1뎁스 .bak → tmp → rename.
/// state::save 패턴과 동일, .bak만 추가.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap_or(path))?;
    if path.exists() {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let _ = std::fs::copy(path, path.with_file_name(format!("{name}.bak")));
        }
    }
    let tmp = path.with_extension("tmp-write");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

/// 디렉터리 파일 목록 (mtime 역순). 없으면 빈 벡터.
pub fn list(root: &Path) -> Vec<FileEntry> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<FileEntry> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.ends_with(".bak") || name.ends_with(".tmp-write") {
                return None;
            }
            let meta = e.metadata().ok()?;
            Some(FileEntry {
                project_id: project_id_from_name(&name),
                name,
                size: meta.len(),
                mtime_epoch: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            })
        })
        .collect();
    out.sort_by_key(|f| std::cmp::Reverse(f.mtime_epoch));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wk-fsutil-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn resolve_rejects_traversal_and_separators() {
        let root = tmpdir("reject");
        for bad in [
            "../state.json",
            "reports/../profile.yaml",
            "a/b",
            "a\\b",
            "/etc/passwd",
            "",
            "C:\\x",
            "a..b", // '..' 포함은 전부 거부 (보수적)
        ] {
            assert!(resolve(&root, bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn resolve_accepts_plain_names() {
        let root = tmpdir("accept");
        let p = resolve(&root, "2026-09-01-scan.md").unwrap();
        assert_eq!(p, root.join("2026-09-01-scan.md"));
    }

    #[test]
    fn project_id_extracted_from_filename() {
        assert_eq!(
            project_id_from_name("2026-09-02-158080-submit.md").as_deref(),
            Some("158080"),
            "날짜는 자릿수로 걸러진다"
        );
        assert_eq!(
            project_id_from_name("158092-epiccounty-proposal.md").as_deref(),
            Some("158092")
        );
        assert_eq!(project_id_from_name("portfolio-entries.md"), None);
        assert_eq!(
            project_id_from_name("2026-09-02-1117.md"),
            None,
            "리포트 파일명"
        );
    }

    #[test]
    fn atomic_write_creates_bak_and_dirs() {
        let root = tmpdir("atomic");
        let target = root.join("nested").join("f.txt");
        atomic_write(&target, b"v1").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"v1");
        atomic_write(&target, b"v2").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"v2");
        assert_eq!(
            std::fs::read(root.join("nested").join("f.txt.bak")).unwrap(),
            b"v1",
            "one-deep bak keeps previous content"
        );
        atomic_write(&target, b"v3").unwrap();
        assert_eq!(
            std::fs::read(root.join("nested").join("f.txt.bak")).unwrap(),
            b"v2",
            "bak is one-deep, overwritten each save"
        );
        assert!(!target.with_extension("tmp-write").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
