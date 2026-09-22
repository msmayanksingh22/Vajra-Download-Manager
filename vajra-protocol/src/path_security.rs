//! Path security and filename sanitization utilities.

use std::path::{Component, Path, PathBuf};

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Sanitizes a raw filename string to prevent path traversal, drive injection,
/// Windows device name collision, and invalid characters.
pub fn sanitize_filename(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "download".to_string();
    }

    // 1. Extract only the last path component (split on both / and \)
    let basename = trimmed
        .split(['/', '\\'])
        .rfind(|s| !s.is_empty() && *s != "." && *s != "..")
        .unwrap_or("download");

    // 2. Remove drive letter prefix if present (e.g. C:foo -> foo)
    let without_drive = if basename.len() >= 2
        && basename.as_bytes()[1] == b':'
        && basename.as_bytes()[0].is_ascii_alphabetic()
    {
        &basename[2..]
    } else {
        basename
    };

    // 3. Replace invalid filesystem characters (< > : " / \ | ? * and control chars)
    let sanitized: String = without_drive
        .chars()
        .map(|c| {
            if c < ' '
                || c == '\x7F'
                || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                c
            }
        })
        .collect();

    // 4. Trim leading/trailing dots and spaces
    let mut clean = sanitized.trim().trim_matches('.').trim().to_string();

    if clean.is_empty() || clean == "." || clean == ".." {
        clean = "download".to_string();
    }

    // 5. Windows reserved device check (case-insensitive for stem)
    let stem = clean.split('.').next().unwrap_or(&clean);
    if WINDOWS_RESERVED
        .iter()
        .any(|&res| res.eq_ignore_ascii_case(stem))
    {
        clean = format!("_{clean}");
    }

    // 6. Max length limit (truncate to 240 chars while keeping extension if possible)
    if clean.len() > 240 {
        if let Some(dot_idx) = clean.rfind('.') {
            let ext = &clean[dot_idx..];
            if ext.len() < 20 {
                let keep_stem_len = 240 - ext.len();
                clean = format!("{}{}", &clean[..keep_stem_len], ext);
            } else {
                clean.truncate(240);
            }
        } else {
            clean.truncate(240);
        }
    }

    clean
}

/// Safely resolves a child file within a base directory.
/// Ensures the resolved path is strictly a descendant of `base_dir`.
pub fn safe_resolve_child(base_dir: &Path, filename: &str) -> Result<PathBuf, String> {
    let clean_name = sanitize_filename(filename);
    let candidate = base_dir.join(&clean_name);

    // Verify candidate does not escape base_dir
    for comp in Path::new(&clean_name).components() {
        match comp {
            Component::Normal(_) => {}
            _ => return Err(format!("Unsafe filename '{filename}' attempted traversal")),
        }
    }

    Ok(candidate)
}

/// Resolves the actual destination path respecting DuplicateAction.
/// If `AutoRename`, appends (1), (2), etc. if the file already exists on disk.
pub fn resolve_duplicate_path(
    dest_dir: &Path,
    filename: &str,
    action: crate::DuplicateAction,
) -> Result<(PathBuf, String), String> {
    let clean = sanitize_filename(filename);
    let initial = dest_dir.join(&clean);

    match action {
        crate::DuplicateAction::Overwrite => Ok((initial, clean)),
        crate::DuplicateAction::Prompt => {
            if initial.exists() {
                Err(format!(
                    "File '{}' already exists and policy is Prompt",
                    clean
                ))
            } else {
                Ok((initial, clean))
            }
        }
        crate::DuplicateAction::AutoRename => {
            if !initial.exists() {
                return Ok((initial, clean));
            }

            let (stem, ext) = match clean.rfind('.') {
                Some(idx) if idx > 0 => (&clean[..idx], Some(&clean[idx + 1..])),
                _ => (clean.as_str(), None),
            };

            for i in 1..=9999 {
                let candidate_name = match ext {
                    Some(e) => format!("{stem} ({i}).{e}"),
                    None => format!("{stem} ({i})"),
                };
                let candidate_path = dest_dir.join(&candidate_name);
                if !candidate_path.exists() {
                    return Ok((candidate_path, candidate_name));
                }
            }

            // Fallback with UUID if all exist
            let unique_name = format!("{stem}_{}", uuid::Uuid::new_v4());
            let candidate_path = dest_dir.join(&unique_name);
            Ok((candidate_path, unique_name))
        }
    }
}

/// Returns true if the path targets a sensitive OS system directory, root filesystem,
/// autostart / startup directory, or credential storage.
pub fn is_unsafe_system_dir(p: &Path) -> bool {
    if is_unsafe_system_dir_normalized(p) {
        return true;
    }

    // If the path exists on disk, canonicalize to resolve symlinks / 8.3 aliases
    if p.exists() {
        if let Ok(canon) = std::fs::canonicalize(p) {
            if is_unsafe_system_dir_normalized(&canon) {
                return true;
            }
        }
    }

    false
}

fn is_unsafe_system_dir_normalized(p: &Path) -> bool {
    let raw_s = p.to_string_lossy().to_lowercase().replace('/', "\\");
    let mut s = raw_s.as_str();

    // Strip Windows extended-length prefix (\\?\, \\.\, \??\)
    if let Some(rest) = s.strip_prefix("\\\\?\\") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("\\\\.\\") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("\\??\\") {
        s = rest;
    }
    // Also handle unc\ if present after extended length prefix
    if let Some(rest) = s.strip_prefix("unc\\") {
        s = rest;
    }

    // Trim trailing dots, slashes, spaces (e.g. C:\Windows. -> C:\Windows)
    let trimmed = s
        .trim()
        .trim_end_matches(['\\', '/'])
        .trim_end_matches('.')
        .trim();
    if trimmed.is_empty() {
        return true;
    }

    // 1. Root directories of any drive (e.g. "c:", "c:\", "d:", "\", "/")
    if trimmed == "\\" || trimmed == "/" {
        return true;
    }
    if trimmed.len() == 2 && trimmed.ends_with(':') && trimmed.as_bytes()[0].is_ascii_alphabetic() {
        return true;
    }

    // 2. Windows system directories (including 8.3 short name aliases)
    if trimmed == "c:\\windows"
        || trimmed.starts_with("c:\\windows\\")
        || trimmed == "c:\\program files"
        || trimmed.starts_with("c:\\program files\\")
        || trimmed == "c:\\program files (x86)"
        || trimmed.starts_with("c:\\program files (x86)\\")
        || trimmed == "c:\\progra~1"
        || trimmed.starts_with("c:\\progra~1\\")
        || trimmed == "c:\\progra~2"
        || trimmed.starts_with("c:\\progra~2\\")
        || trimmed == "c:\\programdata"
        || trimmed.starts_with("c:\\programdata\\")
    {
        return true;
    }

    // Dynamic environment check for Windows system folders if on Windows
    #[cfg(windows)]
    {
        for env_var in &[
            "SystemRoot",
            "windir",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
        ] {
            if let Ok(val) = std::env::var(env_var) {
                let norm_val = val.to_lowercase().replace('/', "\\");
                let norm_trimmed = norm_val.trim().trim_end_matches('\\');
                if !norm_trimmed.is_empty()
                    && (trimmed == norm_trimmed
                        || trimmed.starts_with(&format!("{norm_trimmed}\\")))
                {
                    return true;
                }
            }
        }
    }

    // 3. Windows Autostart / Startup directories
    // User startup: %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup
    // Common startup: %PROGRAMDATA%\Microsoft\Windows\Start Menu\Programs\Startup
    if trimmed.contains("\\start menu\\programs\\startup")
        || (trimmed.ends_with("\\startup") && trimmed.contains("\\programs\\"))
    {
        return true;
    }

    // 4. Linux / Unix autostart directories
    let unix_s = p.to_string_lossy().to_lowercase().replace('\\', "/");
    let unix_trimmed = unix_s.trim().trim_end_matches('/');
    if unix_trimmed.contains("/.config/autostart")
        || unix_trimmed.starts_with("/etc/xdg/autostart")
        || unix_trimmed == "/etc/xdg/autostart"
    {
        return true;
    }

    // 5. Unix system directories
    if unix_trimmed == "/etc"
        || unix_trimmed.starts_with("/etc/")
        || unix_trimmed == "/usr"
        || unix_trimmed.starts_with("/usr/")
        || unix_trimmed == "/bin"
        || unix_trimmed.starts_with("/bin/")
        || unix_trimmed == "/sbin"
        || unix_trimmed.starts_with("/sbin/")
        || unix_trimmed == "/lib"
        || unix_trimmed.starts_with("/lib/")
        || unix_trimmed == "/lib64"
        || unix_trimmed.starts_with("/lib64/")
        || unix_trimmed == "/boot"
        || unix_trimmed.starts_with("/boot/")
        || unix_trimmed == "/dev"
        || unix_trimmed.starts_with("/dev/")
        || unix_trimmed == "/proc"
        || unix_trimmed.starts_with("/proc/")
        || unix_trimmed == "/sys"
        || unix_trimmed.starts_with("/sys/")
        || unix_trimmed == "/root"
        || unix_trimmed.starts_with("/root/")
        || unix_trimmed == "/var/run"
        || unix_trimmed.starts_with("/var/run/")
    {
        return true;
    }

    // 6. Sensitive credential / secret directories in user profiles
    if trimmed.contains("\\.ssh")
        || trimmed.contains("\\.aws")
        || trimmed.contains("\\.gnupg")
        || unix_trimmed.contains("/.ssh")
        || unix_trimmed.contains("/.aws")
        || unix_trimmed.contains("/.gnupg")
    {
        return true;
    }

    false
}

/// Validates and canonicalizes an externally supplied or configured output directory.
/// Enforces:
/// 1. Rejection of empty paths.
/// 2. Rejection of traversal components (`..`) or relative paths attempting escape.
/// 3. Normalization and canonicalization.
/// 4. Rejection of system, root, Startup/autostart, and credential directories via `is_unsafe_system_dir`.
/// 5. Rejection of bare user home profile roots directly (requiring a subfolder like Downloads or Documents).
pub fn validate_output_dir(raw_path: &Path) -> Result<PathBuf, String> {
    let s = raw_path.to_string_lossy();
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("Output directory cannot be empty".to_string());
    }

    // 1. Check for path traversal components
    for comp in raw_path.components() {
        if matches!(comp, Component::ParentDir) {
            return Err(format!(
                "Path traversal ('..') is not permitted in output directory: {}",
                raw_path.display()
            ));
        }
    }

    // 2. Enforce unsafe system / startup directory rejection on raw input
    if is_unsafe_system_dir(raw_path) {
        return Err(format!(
            "Output directory '{}' is a protected system or autostart directory",
            raw_path.display()
        ));
    }

    // 3. Resolve relative paths against standard download dir if relative
    let resolved = if raw_path.is_relative() {
        let base = dirs_next::download_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(raw_path)
    } else {
        raw_path.to_path_buf()
    };

    // 4. Enforce unsafe system / startup directory rejection on resolved path
    if is_unsafe_system_dir(&resolved) {
        return Err(format!(
            "Output directory '{}' is a protected system or autostart directory",
            resolved.display()
        ));
    }

    // 4. Reject bare user profile root (e.g. C:\Users\Username or /home/username)
    // To prevent polluting or overwriting root user directories
    if let Some(home) = dirs_next::home_dir() {
        let norm_home = home.to_string_lossy().to_lowercase().replace('/', "\\");
        let norm_res = resolved.to_string_lossy().to_lowercase().replace('/', "\\");
        if norm_res.trim_end_matches('\\') == norm_home.trim_end_matches('\\') {
            return Err(format!(
                "Cannot use user profile root '{}' directly as output directory; please specify a subdirectory (e.g. Downloads)",
                resolved.display()
            ));
        }
    }

    Ok(resolved)
}

/// Returns the dedicated, controlled WebDAV root directory for a given validated output directory.
/// Decouples WebDAV root so it is strictly isolated inside a dedicated `webdav_shared` subfolder.
pub fn get_controlled_webdav_root(base_output_dir: &Path) -> Result<PathBuf, String> {
    let valid_base = validate_output_dir(base_output_dir)?;
    let dav_root = valid_base.join("webdav_shared");
    if is_unsafe_system_dir(&dav_root) {
        return Err(format!(
            "WebDAV root '{}' violates directory security policy",
            dav_root.display()
        ));
    }
    Ok(dav_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_unsafe_system_dir() {
        assert!(is_unsafe_system_dir(Path::new("C:\\")));
        assert!(is_unsafe_system_dir(Path::new("C:\\Windows")));
        assert!(is_unsafe_system_dir(Path::new("C:\\Windows\\System32")));
        assert!(is_unsafe_system_dir(Path::new(
            "c:/windows/system32/cmd.exe"
        )));
        assert!(is_unsafe_system_dir(Path::new("/etc")));
        assert!(is_unsafe_system_dir(Path::new("/etc/passwd")));
        assert!(!is_unsafe_system_dir(Path::new("D:\\Downloads")));
        assert!(!is_unsafe_system_dir(Path::new(
            "C:\\Users\\User\\Downloads"
        )));
    }

    #[test]
    fn test_sanitize_filename_traversal() {
        assert_eq!(sanitize_filename("../../evil.exe"), "evil.exe");
        assert_eq!(sanitize_filename("..\\..\\evil.exe"), "evil.exe");
        assert_eq!(
            sanitize_filename("C:\\Windows\\System32\\evil.exe"),
            "evil.exe"
        );
        assert_eq!(sanitize_filename("/var/log/syslog"), "syslog");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename("../"), "download");
        assert_eq!(sanitize_filename("..."), "download");
        assert_eq!(sanitize_filename(""), "download");
    }

    #[test]
    fn test_sanitize_filename_reserved_windows() {
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_filename("con.tar.gz"), "_con.tar.gz");
        assert_eq!(sanitize_filename("PRN"), "_PRN");
        assert_eq!(sanitize_filename("aux.mp4"), "_aux.mp4");
        assert_eq!(sanitize_filename("NUL"), "_NUL");
        assert_eq!(sanitize_filename("com1.zip"), "_com1.zip");
        assert_eq!(sanitize_filename("LPT3.doc"), "_LPT3.doc");
    }

    #[test]
    fn test_sanitize_filename_invalid_chars() {
        assert_eq!(
            sanitize_filename("hello:world?*<>.txt"),
            "hello_world____.txt"
        );
        assert_eq!(sanitize_filename("foo\"bar|baz.zip"), "foo_bar_baz.zip");
    }

    #[test]
    fn test_safe_resolve_child() {
        let base = Path::new("C:\\Users\\User\\Downloads");
        let resolved = safe_resolve_child(base, "../../evil.exe").unwrap();
        assert_eq!(resolved, Path::new("C:\\Users\\User\\Downloads\\evil.exe"));

        let resolved_clean = safe_resolve_child(base, "normal_file.pdf").unwrap();
        assert_eq!(
            resolved_clean,
            Path::new("C:\\Users\\User\\Downloads\\normal_file.pdf")
        );
    }

    #[test]
    fn test_resolve_duplicate_path_autorename() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path();

        let (first_path, first_name) =
            resolve_duplicate_path(path, "video.mp4", crate::DuplicateAction::AutoRename).unwrap();
        assert_eq!(first_name, "video.mp4");
        std::fs::write(&first_path, b"test").unwrap();

        let (second_path, second_name) =
            resolve_duplicate_path(path, "video.mp4", crate::DuplicateAction::AutoRename).unwrap();
        assert_eq!(second_name, "video (1).mp4");
        std::fs::write(&second_path, b"test2").unwrap();

        let (third_path, third_name) =
            resolve_duplicate_path(path, "video.mp4", crate::DuplicateAction::AutoRename).unwrap();
        assert_eq!(third_name, "video (2).mp4");
        assert_eq!(third_path, path.join("video (2).mp4"));
    }

    #[test]
    fn test_validate_output_dir_security() {
        // 1. System32 and Windows folders must be rejected (including extended paths, shortnames, trailing dots)
        assert!(validate_output_dir(Path::new("C:\\Windows\\System32")).is_err());
        assert!(validate_output_dir(Path::new("C:\\Windows")).is_err());
        assert!(validate_output_dir(Path::new("C:\\Program Files")).is_err());
        assert!(validate_output_dir(Path::new(r"\\?\C:\Windows")).is_err());
        assert!(validate_output_dir(Path::new(r"\\?\C:\Windows\System32")).is_err());
        assert!(validate_output_dir(Path::new(r"\\.\C:\Windows")).is_err());
        assert!(validate_output_dir(Path::new(r"C:\Windows.")).is_err());
        assert!(validate_output_dir(Path::new(r"C:\PROGRA~1")).is_err());
        assert!(validate_output_dir(Path::new(r"C:\PROGRA~2")).is_err());

        // 2. Windows Startup directories must be rejected
        assert!(validate_output_dir(Path::new(
            "C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup"
        ))
        .is_err());
        assert!(validate_output_dir(Path::new(
            "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Startup"
        ))
        .is_err());

        // 3. Traversal / relative attempts must be rejected
        assert!(validate_output_dir(Path::new("../../evil")).is_err());
        assert!(validate_output_dir(Path::new("..\\..\\evil")).is_err());
        assert!(
            validate_output_dir(Path::new("C:\\Users\\User\\Downloads\\..\\..\\Windows")).is_err()
        );

        // 4. Linux equivalents must be rejected
        assert!(validate_output_dir(Path::new("/etc")).is_err());
        assert!(validate_output_dir(Path::new("/etc/xdg/autostart")).is_err());
        assert!(validate_output_dir(Path::new("/home/user/.config/autostart")).is_err());
        assert!(validate_output_dir(Path::new("/root")).is_err());

        // 5. Legitimate directories must succeed
        let temp = tempfile::tempdir().unwrap();
        let valid_path = temp.path().join("VajraDownloads");
        let validated = validate_output_dir(&valid_path).unwrap();
        assert_eq!(validated, valid_path);

        // 6. Controlled WebDAV root must be dedicated subdirectory
        let dav_root = get_controlled_webdav_root(&valid_path).unwrap();
        assert_eq!(dav_root, valid_path.join("webdav_shared"));
    }
}
