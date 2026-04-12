#!/bin/bash
set -e

PR_NUM="$1"
PR_TITLE="$2"
OPENROUTER_KEY="$3"
TRIGGER="$4"
MODEL="${5:-openrouter/free}"

# 1 Get PR diff
PR_DIFF=$(gh pr diff "$PR_NUM")

# 2 Load prompt
PROMPT_CONTENT=$(cat .github/workflows/claude-review-prompt.md)

# 3 Build full prompt with diff
FULL_PROMPT="$PROMPT_CONTENT

---

$PR_DIFF"

# 4 Call OpenRouter
JSON_PAYLOAD=$(jq -n \
  --arg prompt "$FULL_PROMPT" \
  --arg model "$MODEL" \
  '{"messages": [{"role": "user", "content": $prompt}], "model": $model}')

RESPONSE=$(curl -s --max-time 120 -X POST \
  "https://openrouter.ai/api/v1/chat/completions" \
  -H "Authorization: Bearer $OPENROUTER_KEY" \
  -H "Content-Type: application/json" \
  -H "HTTP-Referer: https://github.com" \
  -H "X-Title: OpenGravity" \
  -d "$JSON_PAYLOAD")

# 5 Check for API errors
ERROR_CODE=$(echo "$RESPONSE" | jq -r '.error.code // empty')
ERROR_MSG=$(echo "$RESPONSE" | jq -r '.error.message // empty')

if [ -n "$ERROR_MSG" ]; then
  case "$ERROR_CODE" in
    "rate_limit_exceeded"|"insufficient_quota"|"daily_limit_reached")
      gh pr comment "$PR_NUM" --body "⚠️ Claude review failed: $ERROR_MSG

El límite de la API gratuita se ha alcanzado. Por favor, inténtalo de nuevo más tarde."
      ;;
    *)
      gh pr comment "$PR_NUM" --body "Error from Claude API: $ERROR_MSG"
      ;;
  esac
  exit 1
fi

# 6 Extract response
CONTENT=$(echo "$RESPONSE" | jq -r '.choices[0].message.content // empty')

# 7 Check response
if [ -z "$CONTENT" ]; then
  gh pr comment "$PR_NUM" --body "Error: Claude review failed to generate response. Please try again."
  exit 1
fi

# 8 Post comment
if [ "$TRIGGER" == "comment" ]; then
  COMMENT_BODY="Review by Claude:

$CONTENT"
else
  COMMENT_BODY="$PR_TITLE

Review by Claude:

$CONTENT"
fi

gh pr comment "$PR_NUM" --body "$(printf '%s' "$COMMENT_BODY")"