use chrono::Local;

pub fn execute(_input: &str) -> Result<String, String> {
    let now_utc = chrono::Utc::now();
    let now_local = Local::now();

    Ok(format!(
        "UTC: {} | Local: {}",
        now_utc.to_rfc3339(),
        now_local.to_rfc3339()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_time_format() {
        let res = execute("").unwrap();
        assert!(res.contains("UTC: "));
        assert!(res.contains(" | Local: "));
    }
}
