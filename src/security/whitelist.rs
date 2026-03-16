use std::collections::HashSet;

pub struct Whitelist {
    allowed_ids: HashSet<u64>,
}

impl Whitelist {
    pub fn new(ids: Vec<u64>) -> Self {
        Self {
            allowed_ids: ids.into_iter().collect(),
        }
    }

    pub fn is_allowed(&self, user_id: u64) -> bool {
        self.allowed_ids.contains(&user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelist_allow() {
        let wl = Whitelist::new(vec![123, 456]);
        assert!(wl.is_allowed(123));
        assert!(wl.is_allowed(456));
    }

    #[test]
    fn test_whitelist_deny() {
        let wl = Whitelist::new(vec![123, 456]);
        assert!(!wl.is_allowed(789));
        assert!(!wl.is_allowed(0));
    }
}
