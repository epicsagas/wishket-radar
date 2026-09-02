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

/// 공고 ID로 쓸 수 있는 디렉터리 이름인지 (숫자 5~7자리).
fn is_project_dir(name: &str) -> bool {
    (5..=7).contains(&name.len()) && name.chars().all(|c| c.is_ascii_digit())
}

/// 파일명이 루트 밖으로 나가지 않는지 검증하고 최종 경로를 반환한다.
/// 이름에 경로 구분자/상위 참조가 있으면 거부한다.
pub fn resolve(root: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.contains('\\')
        || name.contains("..")
        || name.contains(':')
        || name.contains('\0')
    {
        return Err(format!("invalid file name: {name:?}"));
    }
    // "{공고ID}/{파일}" 한 단계만 허용한다. 그 외 슬래시는 거부.
    let mut parts = name.split('/');
    let joined = match (parts.next(), parts.next(), parts.next()) {
        (Some(f), None, _) if !f.is_empty() => root.join(f),
        (Some(d), Some(f), None) if is_project_dir(d) && !f.is_empty() => root.join(d).join(f),
        _ => return Err(format!("invalid file name: {name:?}")),
    };
    // 심볼릭 링크로 루트 밖을 가리키지 않는지 확인한다.
    // 부모는 루트이거나 루트 바로 아래 공고 디렉터리여야 한다.
    if let Some(parent) = joined.parent() {
        let real_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let real_parent = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        let ok = real_parent == real_root
            || real_parent
                .parent()
                .map(|g| g == real_root)
                .unwrap_or(false);
        if !ok {
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
///
/// `{root}/{공고ID}/파일` 구조를 한 단계 훑는다. 소유자는 디렉터리가 정하며
/// 파일명은 해석하지 않는다 — 파일명에서 ID를 추측하면 날짜(202609…) 같은
/// 숫자에 걸려 오분류된다. 루트에 바로 있는 파일은 공고 미상("기타").
pub fn list(root: &Path) -> Vec<FileEntry> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<FileEntry> = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            if !is_project_dir(&name) {
                continue; // 공고 ID 형태가 아닌 디렉터리는 무시
            }
            let Ok(inner) = std::fs::read_dir(e.path()) else {
                continue;
            };
            for f in inner.flatten() {
                let fname = f.file_name().to_string_lossy().into_owned();
                if skip_file(&fname) {
                    continue;
                }
                let Ok(fmeta) = f.metadata() else { continue };
                if fmeta.is_dir() {
                    continue;
                }
                out.push(FileEntry {
                    // 경로는 "{id}/{파일}" — resolve가 이 형태를 허용한다
                    name: format!("{name}/{fname}"),
                    size: fmeta.len(),
                    mtime_epoch: mtime(&fmeta),
                    project_id: Some(name.clone()),
                });
            }
        } else {
            if skip_file(&name) {
                continue;
            }
            out.push(FileEntry {
                name,
                size: meta.len(),
                mtime_epoch: mtime(&meta),
                project_id: None,
            });
        }
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.mtime_epoch));
    out
}

fn skip_file(name: &str) -> bool {
    name.starts_with('.') || name.ends_with(".bak") || name.ends_with(".tmp-write")
}

fn mtime(m: &std::fs::Metadata) -> u64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
    fn list_groups_by_directory_not_filename() {
        // 소유자는 디렉터리가 정한다. 파일명에서 ID를 추측하면
        // "2026-09-02-..."의 202609 같은 숫자에 걸려 오분류된다.
        let root = tmpdir("group");
        std::fs::create_dir_all(root.join("158080")).unwrap();
        std::fs::write(root.join("158080").join("2026-09-02-form.txt"), b"x").unwrap();
        std::fs::write(root.join("158080").join("submit.md"), b"x").unwrap();
        std::fs::write(root.join("memo.md"), b"x").unwrap();
        // 공고 ID 형태가 아닌 디렉터리는 무시
        std::fs::create_dir_all(root.join("drafts")).unwrap();
        std::fs::write(root.join("drafts").join("ignored.md"), b"x").unwrap();

        let files = list(&root);
        let owned: Vec<_> = files.iter().filter(|f| f.project_id.is_some()).collect();
        assert_eq!(owned.len(), 2, "{files:?}");
        assert!(owned
            .iter()
            .all(|f| f.project_id.as_deref() == Some("158080")));
        assert!(owned.iter().any(|f| f.name == "158080/submit.md"));

        let loose: Vec<_> = files.iter().filter(|f| f.project_id.is_none()).collect();
        assert_eq!(loose.len(), 1, "루트 파일은 기타");
        assert_eq!(loose[0].name, "memo.md");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_allows_one_project_dir_level() {
        let root = tmpdir("nested");
        std::fs::create_dir_all(root.join("158080")).unwrap();
        assert_eq!(
            resolve(&root, "158080/a.md").unwrap(),
            root.join("158080").join("a.md")
        );
        // 두 단계 이상, 비ID 디렉터리, 순회는 전부 거부
        assert!(resolve(&root, "158080/sub/a.md").is_err());
        assert!(resolve(&root, "drafts/a.md").is_err());
        assert!(resolve(&root, "../state.json").is_err());
        assert!(resolve(&root, "158080/../../x").is_err());
        assert!(resolve(&root, "158080/").is_err());
        let _ = std::fs::remove_dir_all(&root);
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
