//! 锁定 FAT 库的名称与 create-new 边界。这里只提供相对 DATA 路径，不是 shell 路径解析器。
use fatfs::{DefaultTimeProvider, Dir, File, LossyOemCpConverter, ReadWriteSeek};

pub type Directory<'a, IO> = Dir<'a, IO, DefaultTimeProvider, LossyOemCpConverter>;
pub type OpenFile<'a, IO> = File<'a, IO, DefaultTimeProvider, LossyOemCpConverter>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Length,
    Character,
    Component,
    Path,
}

pub fn validate_component(name: &str) -> Result<(), NameError> {
    // 上游 validate_long_name 使用 UTF-8 字节数，并拒绝 BMP 之外字符。
    if name.is_empty() || name.len() > 255 {
        return Err(NameError::Length);
    }
    if matches!(name, "." | "..") || name.ends_with([' ', '.']) {
        return Err(NameError::Component);
    }
    if name
        .chars()
        .any(|c| c.is_control() || c as u32 > 0xffff || "\"*/:<>?\\|".contains(c))
    {
        return Err(NameError::Character);
    }
    Ok(())
}
pub fn validate_path(path: &str) -> Result<(), NameError> {
    if path.len() > 1024 || path.split('/').count() > 8 {
        return Err(NameError::Path);
    }
    for component in path.split('/') {
        validate_component(component)?;
    }
    Ok(())
}

/// 单 BSP 同步调用；检查与创建之间没有其他写入者，不能直接换成上游 create_file。
pub fn create_new<'a, IO: ReadWriteSeek>(
    directory: &Directory<'a, IO>,
    path: &str,
) -> Result<OpenFile<'a, IO>, fatfs::Error<IO::Error>> {
    validate_path(path).map_err(|e| match e {
        NameError::Length => fatfs::Error::InvalidFileNameLength,
        NameError::Character => fatfs::Error::UnsupportedFileNameCharacter,
        _ => fatfs::Error::InvalidInput,
    })?;
    // 使用库本身的 Unicode 大写比较与短名查找，避免另造一套等价性规则。
    match directory.open_file(path) {
        Ok(_) => return Err(fatfs::Error::AlreadyExists),
        Err(fatfs::Error::NotFound) => (),
        Err(fatfs::Error::InvalidInput) => {
            return Err(match directory.open_dir(path) {
                Ok(_) => fatfs::Error::AlreadyExists,
                Err(error) => error,
            });
        }
        Err(error) => return Err(error),
    }
    directory.create_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    #[test]
    fn names_follow_pinned_byte_limit_and_preserve_bmp() {
        for valid in [
            "萝卜 文件.txt",
            " hello.txt",
            "é.txt",
            "e\u{301}.txt",
            "a;b.txt",
        ] {
            assert_eq!(validate_component(valid), Ok(()));
        }
        assert!(validate_component(&"a".repeat(255)).is_ok());
        assert!(validate_component(&"卜".repeat(85)).is_ok());
        assert_eq!(validate_component(&"a".repeat(256)), Err(NameError::Length));
        assert_eq!(validate_component(&"卜".repeat(86)), Err(NameError::Length));
        assert_eq!(validate_component("🦀.txt"), Err(NameError::Character));
        for invalid in [
            "", ".", "..", "x.", "x ", "a/b", "a\\b", "x:y", "a\0b", "a\u{7f}b", "a\u{85}b",
        ] {
            assert!(validate_component(invalid).is_err(), "{invalid:?}");
        }
    }
    #[test]
    fn relative_path_cannot_escape_or_silently_normalize() {
        assert!(validate_path("folder/萝卜.txt").is_ok());
        for invalid in ["/data/x", "a//b", "a/", "../b", "a/./b", "a/../b"] {
            assert!(validate_path(invalid).is_err(), "{invalid}");
        }
        assert!(validate_path(&("a/".repeat(8) + "x")).is_err());
        assert!(validate_path(&("a".repeat(255) + &"/bbb".repeat(255))).is_err());
        assert_eq!(validate_component(&"ReadMe.txt".to_string()), Ok(()));
    }
}
