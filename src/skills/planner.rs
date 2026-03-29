use tracing::debug;

const MULTI_STEP_CONNECTORS: &[&str] = &["y después", "luego", "primero", "después", "también"];

const MAX_PLAN_STEPS: usize = 2;

pub struct Planner;

impl Planner {
    pub fn new() -> Self {
        Self
    }

    pub fn has_multi_step_intent(&self, message: &str) -> bool {
        let message_lower = message.to_lowercase();
        MULTI_STEP_CONNECTORS
            .iter()
            .any(|connector| message_lower.contains(connector))
    }

    pub fn create_plan(&self, user_message: &str) -> Option<Plan> {
        if !self.has_multi_step_intent(user_message) {
            debug!("No multi-step connector found in message");
            return None;
        }

        debug!("Multi-step intent detected in message");
        let steps = self.parse_steps(user_message);

        if steps.is_empty() {
            debug!("No valid steps parsed from message");
            return None;
        }

        let truncated_steps: Vec<PlanStep> = steps.into_iter().take(MAX_PLAN_STEPS).collect();

        debug!("Plan created with {} step(s)", truncated_steps.len());

        Some(Plan {
            steps: truncated_steps,
        })
    }

    fn parse_steps(&self, message: &str) -> Vec<PlanStep> {
        let message_lower = message.to_lowercase();
        let mut steps = Vec::new();

        for connector in MULTI_STEP_CONNECTORS {
            if message_lower.contains(connector) {
                let parts: Vec<&str> = message.split(connector).collect();

                for part in parts {
                    let cleaned = part.trim();
                    if !cleaned.is_empty() {
                        if let Some(tool_name) = self.extract_tool_from_text(cleaned) {
                            if self.is_tool_allowed(&tool_name) {
                                steps.push(PlanStep::Tool(tool_name));
                            } else {
                                debug!("Tool '{}' not in whitelist, rejecting plan", tool_name);
                                return Vec::new();
                            }
                        } else if !cleaned.is_empty() {
                            steps.push(PlanStep::Direct(cleaned.to_string()));
                        }
                    }
                }
                break;
            }
        }

        steps
    }

    fn extract_tool_from_text(&self, text: &str) -> Option<String> {
        let text_lower = text.to_lowercase();

        let known_tools = ["get_current_time", "get_weather"];

        for tool in known_tools {
            if text_lower.contains(tool) {
                return Some(tool.to_string());
            }
        }

        // Check if text contains something that looks like a tool call
        // Pattern: starts with "get_" or contains "tool:" or contains "tool_"
        if text_lower.starts_with("get_")
            || text_lower.contains("tool:")
            || text_lower.contains("tool_")
        {
            // Extract the tool-like name
            let first_word = text_lower.split_whitespace().next().unwrap_or("");
            if first_word.starts_with("get_") || first_word.starts_with("tool") {
                return Some(first_word.to_string());
            }
        }

        None
    }

    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        let known_tools = ["get_current_time", "get_weather"];

        known_tools.contains(&tool_name)
    }

    pub fn normalize_direct_step(&self, content: &str) -> String {
        let connectors_to_remove = [
            "primero",
            "luego",
            "después",
            "y después",
            "también",
            ",",
            ".",
        ];

        let mut result = content.to_lowercase();

        for connector in connectors_to_remove {
            result = result.replace(connector, "");
        }

        let result = result.split_whitespace().collect::<Vec<_>>().join(" ");

        let mut chars = result.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().chain(chars).collect(),
        }
    }

    pub fn split_message(&self, message: &str) -> Option<(String, String)> {
        if !self.has_multi_step_intent(message) {
            return None;
        }

        let message_lower = message.to_lowercase();

        for connector in MULTI_STEP_CONNECTORS {
            if message_lower.contains(connector) {
                let parts: Vec<&str> = message.splitn(2, connector).collect();
                if parts.len() == 2 {
                    let factual = parts[0].trim().to_string();
                    let remaining = parts[1].trim().to_string();
                    if !factual.is_empty() && !remaining.is_empty() {
                        debug!(
                            "Split message: factual='{}', remaining='{}'",
                            factual, remaining
                        );
                        return Some((factual, remaining));
                    }
                }
            }
        }

        None
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub steps: Vec<PlanStep>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn first_step(&self) -> Option<&PlanStep> {
        self.steps.first()
    }

    pub fn remaining_steps(&self) -> Vec<PlanStep> {
        self.steps.iter().skip(1).cloned().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStep {
    Tool(String),
    Direct(String),
}

impl PlanStep {
    pub fn is_tool(&self) -> bool {
        matches!(self, PlanStep::Tool(_))
    }

    pub fn tool_name(&self) -> Option<&str> {
        match self {
            PlanStep::Tool(name) => Some(name),
            PlanStep::Direct(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_step_no_plan() {
        let planner = Planner::new();
        let result = planner.create_plan("Dime la hora");
        assert!(result.is_none(), "Single step should not create a plan");
    }

    #[test]
    fn test_two_step_tool_plus_memory() {
        let planner = Planner::new();
        let result = planner.create_plan("get_current_time y después get_weather");
        assert!(result.is_some());
        let plan = result.unwrap();
        assert_eq!(plan.len(), 2);
        assert!(plan.steps[0].is_tool());
        assert!(plan.steps[1].is_tool());
    }

    #[test]
    fn test_unknown_tool_rejected() {
        let planner = Planner::new();
        let result = planner.create_plan("get_current_time y después get_invalid_tool");
        assert!(result.is_none(), "Unknown tool should cause plan rejection");
    }

    #[test]
    fn test_plan_truncates_more_than_two_steps() {
        let planner = Planner::new();
        let result =
            planner.create_plan("Primero dime la hora y después el clima y después la fecha");
        assert!(result.is_some());
        let plan = result.unwrap();
        assert!(
            plan.len() <= 2,
            "Plan should be truncated to max {} steps, got {}",
            MAX_PLAN_STEPS,
            plan.len()
        );
    }

    #[test]
    fn test_trigger_on_primero() {
        let planner = Planner::new();
        assert!(planner.has_multi_step_intent("Primero haz esto y después lo otro"));
    }

    #[test]
    fn test_trigger_on_luego() {
        let planner = Planner::new();
        assert!(planner.has_multi_step_intent("Haz esto luego haz eso"));
    }

    #[test]
    fn test_trigger_on_y_despues() {
        let planner = Planner::new();
        assert!(planner.has_multi_step_intent("Dime la hora y después el clima"));
    }

    #[test]
    fn test_no_trigger_simple_message() {
        let planner = Planner::new();
        assert!(!planner.has_multi_step_intent("¿Qué hora es?"));
    }

    #[test]
    fn test_plan_first_step() {
        let planner = Planner::new();
        let result = planner.create_plan("Dime la hora y después el clima");
        assert!(result.is_some());
        let plan = result.unwrap();
        assert!(plan.first_step().is_some());
    }

    #[test]
    fn test_plan_remaining_steps() {
        let planner = Planner::new();
        let result = planner.create_plan("Dime la hora y después el clima y después la fecha");
        assert!(result.is_some());
        let plan = result.unwrap();
        let remaining = plan.remaining_steps();
        assert!(remaining.len() <= 1);
    }

    #[test]
    fn test_split_message_factual_and_remaining() {
        let planner = Planner::new();
        let result = planner.split_message("Mi color favorito es verde y después decime la hora");
        assert!(result.is_some());
        let (factual, remaining) = result.unwrap();
        assert_eq!(factual, "Mi color favorito es verde");
        assert_eq!(remaining, "decime la hora");
    }

    #[test]
    fn test_split_message_no_multi_step() {
        let planner = Planner::new();
        let result = planner.split_message("Dime la hora");
        assert!(result.is_none());
    }
}
