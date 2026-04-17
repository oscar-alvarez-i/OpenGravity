use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;

const NOTE_FILE: &str = "local_notes.txt";

thread_local! {
    static NOTES_PATH: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

pub fn set_notes_path(path: std::path::PathBuf) {
    NOTES_PATH.with(|p| *p.borrow_mut() = Some(path));
}

pub fn clear_notes_path() {
    NOTES_PATH.with(|p| *p.borrow_mut() = None);
}

fn resolve_notes_path() -> std::path::PathBuf {
    NOTES_PATH.with(|p| p.borrow().clone()).unwrap_or_else(|| {
        std::env::var("OPEN_GRAVITY_NOTES_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(NOTE_FILE)
            })
    })
}

enum FileMode {
    Read,
    Append,
}

fn with_note_file<F, T>(path: &std::path::Path, mode: FileMode, op: F) -> Result<T, String>
where
    F: FnOnce(&mut std::fs::File) -> Result<T, std::io::Error>,
{
    validate_note_path(path)?;

    let is_production_path = std::env::current_dir()
        .map_err(|e| format!("Failed to get cwd: {}", e))?
        .join(NOTE_FILE)
        == path;

    let mut opts = OpenOptions::new();
    match (mode, is_production_path) {
        (FileMode::Read, _) => {
            opts.read(true).custom_flags(libc::O_NOFOLLOW);
        }
        (FileMode::Append, true) => {
            opts.create(true)
                .read(true)
                .write(true)
                .append(true)
                .custom_flags(libc::O_NOFOLLOW);
        }
        (FileMode::Append, false) => {
            opts.create(true)
                .append(true)
                .custom_flags(libc::O_NOFOLLOW);
        }
    }

    let mut file = opts
        .open(path)
        .map_err(|e| format!("Failed to open file: {}", e))?;

    op(&mut file).map_err(|e| format!("{}", e))
}

fn resolve_note_path() -> Result<std::path::PathBuf, String> {
    Ok(resolve_notes_path())
}

fn validate_note_path(path: &std::path::Path) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get cwd: {}", e))?;
    let fixed_path = cwd.join(NOTE_FILE);
    let is_production_path = path == fixed_path;

    if path.exists() {
        let metadata =
            std::fs::symlink_metadata(path).map_err(|e| format!("Failed to check file: {}", e))?;

        if metadata.file_type().is_symlink() {
            return Err("Cannot access symlink".to_string());
        }

        if !metadata.file_type().is_file() {
            return Err("Target is not a regular file".to_string());
        }

        if is_production_path {
            let canonical = std::fs::canonicalize(path)
                .map_err(|e| format!("Failed to canonicalize: {}", e))?;
            let expected_canonical = std::fs::canonicalize(&fixed_path)
                .map_err(|e| format!("Failed to canonicalize expected: {}", e))?;
            if canonical != expected_canonical {
                return Err("Path escape detected".to_string());
            }
        }
    } else {
        let parent = path.parent().ok_or("Invalid path")?;
        if parent != cwd {
            let tmp = std::env::temp_dir();
            if path.starts_with(&tmp) {
                return Ok(());
            }
            return Err("Invalid target directory".to_string());
        }
    }

    Ok(())
}

fn write_to_path_internal(input: &str, path: &std::path::Path) -> Result<String, String> {
    validate_note_path(path)?;

    with_note_file(path, FileMode::Append, |file| writeln!(file, "{}", input))
        .map_err(|e| format!("Failed to write: {}", e))?;

    Ok("nota guardada".to_string())
}

pub fn execute(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Input cannot be empty".to_string());
    }
    if input.contains('\n')
        || input.contains('\r')
        || input.contains('\u{2028}')
        || input.contains('\u{2029}')
    {
        return Err("Input must be single-line".to_string());
    }

    let path = resolve_note_path()?;
    write_to_path_internal(input, &path)
}

pub fn execute_read(input: &str) -> Result<String, String> {
    let input = input.trim();
    if !input.is_empty() {
        return Err("read_local_notes does not accept input".to_string());
    }

    let path = resolve_note_path()?;

    validate_note_path(&path)?;

    if !path.exists() {
        return Err("File not found".to_string());
    }

    let mut content = String::new();
    with_note_file(&path, FileMode::Read, |file| {
        file.read_to_string(&mut content)
    })
    .map_err(|e| format!("Failed to read file: {}", e))?;

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;

    fn unique_path(suffix: &str) -> std::path::PathBuf {
        let cwd = std::env::current_dir().unwrap();
        cwd.join(format!(
            ".test_{}_{}.txt",
            suffix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn test_write_empty_input_fails() {
        let result = execute("");
        assert!(result.is_err());
    }

    #[test]
    fn test_write_whitespace_input_fails() {
        let result = execute("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_write_multiline_input_fails() {
        let result = execute("line1\nline2");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("single-line"));
    }

    #[test]
    fn test_write_carriage_return_fails() {
        let result = execute("line1\rline2");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("single-line"));
    }

    #[test]
    fn test_write_unicode_line_separators() {
        let result = execute("line1\u{2028}line2");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("single-line"));
    }

    #[test]
    fn test_write_paragraph_separator() {
        let result = execute("line1\u{2029}line2");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("single-line"));
    }

    #[test]
    fn test_write_creates_file() {
        let path = unique_path("creates");
        fs::remove_file(&path).ok();

        let result = write_to_path_internal("test note", &path);
        assert!(result.is_ok());

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("test note"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_appends() {
        let path = unique_path("appends");
        fs::remove_file(&path).ok();

        write_to_path_internal("line 1", &path).ok();
        write_to_path_internal("line 2", &path).ok();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("line 1"));
        assert!(lines[1].contains("line 2"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_single_line_with_spaces() {
        let path = unique_path("spaces");
        fs::remove_file(&path).ok();

        let result = write_to_path_internal("hello world", &path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello world"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_symlink_rejected() {
        let path = unique_path("symlink");
        fs::remove_file(&path).ok();

        let target = unique_path("symlink_target");
        fs::write(&target, "target").ok();
        std::os::unix::fs::symlink(&target, &path).ok();

        let result = write_to_path_internal("should fail", &path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("symlink"));

        fs::remove_file(&target).ok();
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_unicode_content() {
        let path = unique_path("unicode");
        fs::remove_file(&path).ok();

        let unicode_input = "note with émoji 🎉 and 中文";
        let result = write_to_path_internal(unicode_input, &path);
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), unicode_input);

        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_read_existing_file() {
        let path = resolve_note_path().unwrap();
        fs::remove_file(&path).ok();

        fs::write(&path, "line1 content").ok();

        let result = execute_read("");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("line1"));

        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_read_file_not_exists() {
        let path = resolve_note_path().unwrap();
        fs::remove_file(&path).ok();

        let result = execute_read("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("File not found"));
    }

    #[test]
    #[serial]
    fn test_read_empty_file() {
        let path = resolve_note_path().unwrap();
        fs::remove_file(&path).ok();

        fs::write(&path, "").ok();

        let result = execute_read("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());

        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_read_multiple_lines() {
        let path = resolve_note_path().unwrap();
        fs::remove_file(&path).ok();

        fs::write(&path, "first\nsecond\nthird\n").ok();

        let result = execute_read("");
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("first"));
        assert!(content.contains("second"));
        assert!(content.contains("third"));

        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_read_input_not_allowed() {
        let result = execute_read("some input");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not accept input"));
    }

    #[test]
    #[serial]
    fn test_read_symlink_rejected() {
        let path = resolve_note_path().unwrap();
        fs::remove_file(&path).ok();

        let target = std::env::current_dir().unwrap().join(".test_read_target");
        fs::remove_file(&target).ok();
        fs::write(&target, "target content").ok();
        std::os::unix::fs::symlink(&target, &path).ok();

        let result = execute_read("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("symlink"));

        fs::remove_file(&target).ok();
        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_read_invalid_path_outside_cwd() {
        let cwd = std::env::current_dir().unwrap();
        let parent_dir = match cwd.parent() {
            Some(p) => p.to_path_buf(),
            None => cwd.clone(),
        };
        let outside_path = parent_dir.join("local_notes.txt");

        fs::remove_file(&outside_path).ok();

        let validation_result = validate_note_path(&outside_path);
        assert!(validation_result.is_err());
        assert!(validation_result
            .unwrap_err()
            .contains("Invalid target directory"));
    }

    #[test]
    #[serial]
    fn test_read_invalid_path_is_directory() {
        let file_path = resolve_note_path().unwrap();
        fs::remove_file(&file_path).ok();
        fs::create_dir_all(&file_path).unwrap();

        let result = execute_read("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Target is not a regular file"));

        fs::remove_dir_all(&file_path).ok();
    }

    // Note: The following error cases cannot be deterministically simulated in the current architecture:
    // - IO error during write (disk full, permission denied, etc.)
    // - IO error during read (disk full, permission denied, etc.)
    // These would require mocking the filesystem or inducing hardware errors,
    // which is not feasible in unit tests without changing the runtime behavior.
}
