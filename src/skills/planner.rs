use crate::domain::message::Message;
use anyhow::Result;

pub struct SkillPlanner;

impl SkillPlanner {
    pub fn new() -> Self {
        Self
    }

    pub fn plan(&self, _messages: &[Message]) -> Result<SkillPlan> {
        Ok(SkillPlan { steps: Vec::new() })
    }
}

impl Default for SkillPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SkillPlan {
    pub steps: Vec<SkillStep>,
}

#[derive(Debug, Clone)]
pub struct SkillStep {
    pub skill_name: String,
    pub reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_returns_empty_plan() {
        let planner = SkillPlanner::new();
        let result = planner.plan(&[]).unwrap();
        assert!(result.steps.is_empty());
    }
}
