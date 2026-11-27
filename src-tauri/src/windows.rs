//! Windows API 호출 모음.

use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
use windows_sys::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, SetDefaultDllDirectories,
};
use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;

/// DLL을 앱 폴더와 System32에서만 찾게 한다. portable-pty가 `conpty.dll`을 이름만으로
/// 불러오므로, 막지 않으면 현재 폴더나 PATH에 심어 둔 DLL이 로드될 수 있다.
pub fn restrict_dll_search() {
    // SAFETY: 인자는 상수 플래그 하나뿐이다.
    let ok = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) };
    assert!(ok != 0, "DLL 검색 경로를 제한하지 못했어요");
}

/// xterm의 ConPTY 줄바꿈 처리가 Windows 빌드에 따라 달라진다.
pub fn build_number() -> u32 {
    // SAFETY: 크기를 채운 구조체를 넘기며, RtlGetVersion은 그 안에만 쓴다.
    unsafe {
        let mut info: OSVERSIONINFOW = std::mem::zeroed();
        info.dwOSVersionInfoSize = size_of::<OSVERSIONINFOW>() as u32;
        RtlGetVersion(&mut info);
        info.dwBuildNumber
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::LoadLibraryW;

    fn can_load(name: &str) -> bool {
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        // SAFETY: NUL로 끝나는 이름을 넘기고, 불러온 모듈은 바로 해제한다.
        unsafe {
            let module = LoadLibraryW(wide.as_ptr());
            if module.is_null() {
                return false;
            }
            FreeLibrary(module);
            true
        }
    }

    #[test]
    fn dll_in_current_directory_is_not_loaded_after_restriction() {
        let dir = std::env::temp_dir().join(format!("lmux-dll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let system = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap());
        std::fs::copy(
            system.join(r"System32\version.dll"),
            dir.join("lmux_planted.dll"),
        )
        .unwrap();
        let before_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let before = can_load("lmux_planted.dll");
        restrict_dll_search();
        let after = can_load("lmux_planted.dll");

        std::env::set_current_dir(before_dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            before,
            "제한 전에는 현재 폴더에서 로드되어야 검사가 의미 있음"
        );
        assert!(!after);
        assert!(build_number() >= 17763);
    }
}
