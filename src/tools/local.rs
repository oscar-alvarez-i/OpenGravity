use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

const NOTE_FILE: &str = "local_notes.txt";

fn get_note_path() -> PathBuf {
    PathBuf::from(NOTE_FILE)
}

fn write_to_path(input: &str, path: PathBuf) -> Result<String, String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open file: {}", e))?;

    writeln!(file, "{}", input).map_err(|e| format!("Failed to write: {}", e))?;

    Ok("nota guardada".to_string())
}

pub fn execute(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Input cannot be empty".to_string());
    }
    if input.contains('\n') {
        return Err("Input must be single-line".to_string());
    }

    write_to_path(input, get_note_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_write_note_creates_file() {
        let test_file = "test_notes.txt";
        let path = PathBuf::from(test_file);

        let result = write_to_path("test note", path.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "nota guardada");

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("test note"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_note_appends() {
        let test_file = "test_notes_append.txt";
        let path = PathBuf::from(test_file);

        write_to_path("line 1", path.clone()).ok();
        write_to_path("line 2", path.clone()).ok();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("line 1"));
        assert!(lines[1].contains("line 2"));

        fs::remove_file(&path).ok();
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
    fn test_write_single_line_with_spaces() {
        let test_file = "test_notes_spaces.txt";
        let path = PathBuf::from(test_file);

        let result = write_to_path("hello world", path.clone());
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello world"));

        fs::remove_file(&path).ok();
    }
}
