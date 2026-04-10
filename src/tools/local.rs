use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

const NOTE_FILE: &str = "local_notes.txt";

fn write_to_path(input: &str) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get cwd: {}", e))?;
    let path = cwd.join(NOTE_FILE);

    if path.exists() {
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|e| format!("Failed to check file: {}", e))?;

        if metadata.file_type().is_symlink() {
            return Err("Cannot write to symlink".to_string());
        }

        if !metadata.file_type().is_file() {
            return Err("Target is not a regular file".to_string());
        }

        let canonical = std::fs::canonicalize(&path)
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

        let expected = cwd.join(NOTE_FILE);
        let expected_canonical = std::fs::canonicalize(&expected)
            .map_err(|e| format!("Failed to canonicalize expected path: {}", e))?;

        if canonical != expected_canonical {
            return Err("Path escape detected".to_string());
        }
    } else {
        let parent = path.parent().ok_or("Invalid path")?;
        if parent != cwd {
            return Err("Invalid target directory".to_string());
        }
    }

    let mut file = if path.exists() {
        let mut opts = OpenOptions::new();
        opts.read(true)
            .write(true)
            .append(true)
            .custom_flags(libc::O_NOFOLLOW);
        opts.open(&path)
            .map_err(|e| format!("Failed to open file: {}", e))?
    } else {
        let mut opts = OpenOptions::new();
        opts.create(true)
            .append(true)
            .custom_flags(libc::O_NOFOLLOW);
        opts.open(&path)
            .map_err(|e| format!("Failed to create file: {}", e))?
    };

    writeln!(file, "{}", input).map_err(|e| format!("Failed to write: {}", e))?;

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

    write_to_path(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;

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
    #[serial]
    fn test_write_note_creates_file() {
        let path = std::env::current_dir().unwrap().join(NOTE_FILE);
        fs::remove_file(&path).ok();

        let result = write_to_path("test note");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "nota guardada");

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("test note"));

        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_write_note_appends() {
        let path = std::env::current_dir().unwrap().join(NOTE_FILE);
        fs::remove_file(&path).ok();

        write_to_path("line 1").ok();
        write_to_path("line 2").ok();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("line 1"));
        assert!(lines[1].contains("line 2"));

        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_write_single_line_with_spaces() {
        let path = std::env::current_dir().unwrap().join(NOTE_FILE);
        fs::remove_file(&path).ok();

        let result = write_to_path("hello world");
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello world"));

        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_write_symlink_rejected() {
        let path = std::env::current_dir().unwrap().join(NOTE_FILE);
        fs::remove_file(&path).ok();

        let symlink_path = std::env::current_dir()
            .unwrap()
            .join("test_symlink_target.txt");
        fs::write(&symlink_path, "target").ok();
        std::os::unix::fs::symlink(&symlink_path, &path).ok();

        let result = write_to_path("should fail");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("symlink"));

        fs::remove_file(&symlink_path).ok();
        fs::remove_file(&path).ok();
    }

    #[test]
    #[serial]
    fn test_write_unicode_content() {
        let path = std::env::current_dir().unwrap().join(NOTE_FILE);
        fs::remove_file(&path).ok();

        let unicode_input = "note with émoji 🎉 and 中文";
        let result = write_to_path(unicode_input);
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), unicode_input);

        fs::remove_file(&path).ok();
    }
}
