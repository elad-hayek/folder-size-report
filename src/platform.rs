use std::fs::Metadata;
use std::path::Path;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(windows)]
const FILE_ATTRIBUTE_HIDDEN: u32 = windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 =
    windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

pub fn is_hidden(_path: &Path, metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
    }
    #[cfg(not(windows))]
    {
        _path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
    }
}

pub fn is_reparse_point(_path: &Path, metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
