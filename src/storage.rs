//! ファイルの読み取り上限と、設定ストアの排他更新・一時ファイルによる置換。
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// 入力検証・保存形式の不一致など、呼出し側へ表示するエラーを畳む。
pub(crate) fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

/// $HOME。空文字も未設定として扱う。
pub(crate) fn home_dir() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| invalid("HOME が設定されていません"))
}

/// $HOME/.solo/<sub>
pub(crate) fn solo_dir(sub: &str) -> io::Result<PathBuf> {
    Ok(home_dir()?.join(".solo").join(sub))
}

/// ファイルが存在しない場合だけ None。破損・権限・サイズ超過は呼出し元へ返す。
pub(crate) fn read_optional(
    path: &Path,
    max_bytes: u64,
    too_large: &str,
) -> io::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    read_bounded(file, max_bytes, too_large).map(Some)
}

/// metadata の取得後に増えたファイルも、読み取り時の byte 数で制限する。
pub(crate) fn read_text(path: &Path, max_bytes: u64, too_large: &str) -> io::Result<String> {
    let bytes = read_bounded(File::open(path)?, max_bytes, too_large)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// バイナリのまま上限付きで読む。UTF-8 判定は呼出し側に任せる。
pub(crate) fn read_bytes(path: &Path, max_bytes: u64, too_large: &str) -> io::Result<Vec<u8>> {
    read_bounded(File::open(path)?, max_bytes, too_large)
}

fn read_bounded(reader: impl Read, max_bytes: u64, too_large: &str) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(io::Error::new(io::ErrorKind::InvalidData, too_large));
    }
    Ok(bytes)
}

/// 読み直しから保存までロックを保持する。内容の検証と競合判定は各ストアが行う。
pub(crate) struct FileTransaction<'a> {
    path: &'a Path,
    parent: &'a Path,
    lock: File,
}

impl<'a> FileTransaction<'a> {
    pub(crate) fn begin(
        path: &'a Path,
        lock_extension: &str,
        invalid_path: &str,
        busy: &str,
    ) -> io::Result<Self> {
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, invalid_path))?;
        fs::create_dir_all(parent)?;
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension(lock_extension))?;
        lock.try_lock()
            .map_err(|error| io::Error::other(format!("{busy}: {error}")))?;
        Ok(Self { path, parent, lock })
    }

    pub(crate) fn commit(&self, bytes: &[u8]) -> io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(self.parent)?;
        file.write_all(bytes)?;
        file.as_file().sync_all()?;
        file.persist(self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

impl Drop for FileTransaction<'_> {
    fn drop(&mut self) {
        // close だけでは、子プロセスに継承された記述子がロックを保持し続ける。
        let _ = self.lock.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn begin(path: &Path) -> io::Result<FileTransaction<'_>> {
        FileTransaction::begin(path, "lock", "invalid path", "busy")
    }

    #[test]
    fn an_oversized_reader_is_not_drained_before_reporting_the_limit() {
        let data = vec![b'x'; 32 * 1024];
        let mut reader = data.as_slice();
        let error = read_bounded(&mut reader, 8, "too large").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            data.len() - reader.len() <= 9,
            "only one byte beyond the limit is needed to detect overflow"
        );
    }

    #[test]
    fn missing_files_and_read_errors_are_distinct_from_size_limits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings");
        assert!(read_optional(&path, 3, "too large").unwrap().is_none());
        fs::write(&path, "abc").unwrap();
        assert_eq!(
            read_optional(&path, 3, "too large").unwrap().unwrap(),
            b"abc"
        );
        assert_eq!(
            read_optional(&path, 2, "too large").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(read_optional(dir.path(), 3, "too large").is_err());
    }

    #[test]
    fn abandoned_and_busy_transactions_preserve_saved_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private/settings");
        let transaction = begin(&path).unwrap();
        transaction.commit(b"original").unwrap();
        assert!(begin(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        drop(transaction);
        drop(begin(&path).unwrap());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        begin(&path).unwrap().commit(b"updated").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"updated");
    }

    #[cfg(unix)]
    #[test]
    fn transaction_releases_lock_even_while_a_duplicate_handle_lives() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings");
        let transaction = begin(&path).unwrap();
        // 子プロセスへ継承された記述子と同じく、複製は同一ロックを共有する。
        let duplicate = transaction.lock.try_clone().unwrap();
        drop(transaction);
        let next = begin(&path).expect("the completed transaction must release its lock");
        drop(duplicate);
        assert!(
            begin(&path).is_err(),
            "an older handle must not unlock the next writer"
        );
        drop(next);
        assert!(begin(&path).is_ok());
    }
}
