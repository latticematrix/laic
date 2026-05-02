#!/usr/bin/env bash
# LAIC Boundary Check — detects code that may violate the "Mechanism vs Policy" principle.
# Run: ./ci/boundary-check.sh
#
# This script greps for patterns that suggest LAIC is doing things it shouldn't.
# False positives are possible — review flagged lines manually.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

WARNINGS=0

check_pattern() {
  local desc="$1"
  local pattern="$2"
  local matches
  matches=$(grep -rn "$pattern" crates/ --include='*.rs' || true)
  if [ -n "$matches" ]; then
    echo -e "${YELLOW}REVIEW${NC} $desc"
    echo "$matches" | head -10
    echo ""
    WARNINGS=$((WARNINGS + 1))
  fi
}

echo "=== LAIC Boundary Check ==="
echo "Checking for patterns that may violate Mechanism vs Policy..."
echo ""

# Routing/scheduling logic (belongs to Runtime)
check_pattern "Routing/scheduling logic detected" \
  'route\|schedule\|priority_queue\|load_balance\|round_robin'

# Business logic / state management (belongs to application layer)
check_pattern "Business state management detected" \
  'session_state\|user_state\|chat_history\|workflow\|orchestrat'

# Resource allocation (belongs to Runtime/K8s)
check_pattern "Resource allocation detected" \
  'allocate_gpu\|gpu_memory\|resource_pool\|ResourceAllocator'

# Distributed consensus (belongs to etcd/Consul)
check_pattern "Distributed consensus detected" \
  'leader_elect\|consensus\|raft\|paxos\|distributed_lock'

# AI/ML logic (LAIC is transport, not intelligence)
check_pattern "AI/ML logic detected" \
  'inference\|model_load\|embedding\|tokeniz\|llm_call'

echo ""
if [ "$WARNINGS" -eq 0 ]; then
  echo -e "${GREEN}No boundary violations detected.${NC}"
else
  echo -e "${YELLOW}${WARNINGS} pattern(s) flagged for review.${NC}"
  echo "These may be false positives. Verify each against docs/LAIC 绝对边界.md"
fi
