#!/bin/bash

# Real LLM Integration Test Runner
# 
# This script runs integration tests using real LLM API instead of mock data.
# 
# Usage:
#   ./scripts/run-real-llm-tests.sh [qwen|claude|openai|ollama]
#
# Examples:
#   ./scripts/run-real-llm-tests.sh qwen      # Use Qwen API
#   ./scripts/run-real-llm-tests.sh claude    # Use Claude API
#   ./scripts/run-real-llm-tests.sh openai    # Use OpenAI API
#   ./scripts/run-real-llm-tests.sh ollama    # Use local Ollama

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default LLM backend
LLM_BACKEND="${1:-qwen}"

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  Real LLM Integration Test Runner                          ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}\n"

# Setup LLM configuration based on backend
case "$LLM_BACKEND" in
    qwen)
        echo -e "${YELLOW}🤖 Configuring Qwen API...${NC}\n"
        
        if [ -z "$LLM_API_KEY" ]; then
            echo -e "${RED}Error: LLM_API_KEY not set${NC}"
            echo "Please set: export LLM_API_KEY=\"sk-...\""
            exit 1
        fi
        
        export LLM_BACKEND="openai_compatible"
        export LLM_BASE_URL="${LLM_BASE_URL:-https://dashscope.aliyuncs.com/compatible-mode/v1}"
        export LLM_MODEL="${LLM_MODEL:-qwen-max}"
        
        echo "✓ LLM Backend: openai_compatible"
        echo "✓ Base URL: $LLM_BASE_URL"
        echo "✓ Model: $LLM_MODEL"
        echo ""
        ;;
        
    claude)
        echo -e "${YELLOW}🤖 Configuring Claude API...${NC}\n"
        
        if [ -z "$ANTHROPIC_API_KEY" ]; then
            echo -e "${RED}Error: ANTHROPIC_API_KEY not set${NC}"
            echo "Please set: export ANTHROPIC_API_KEY=\"sk-ant-...\""
            exit 1
        fi
        
        export LLM_BACKEND="anthropic"
        export ANTHROPIC_MODEL="${ANTHROPIC_MODEL:-claude-3-5-sonnet-20241022}"
        
        echo "✓ LLM Backend: anthropic"
        echo "✓ Model: $ANTHROPIC_MODEL"
        echo ""
        ;;
        
    openai)
        echo -e "${YELLOW}🤖 Configuring OpenAI API...${NC}\n"
        
        if [ -z "$OPENAI_API_KEY" ]; then
            echo -e "${RED}Error: OPENAI_API_KEY not set${NC}"
            echo "Please set: export OPENAI_API_KEY=\"sk-...\""
            exit 1
        fi
        
        export LLM_BACKEND="openai"
        export OPENAI_MODEL="${OPENAI_MODEL:-gpt-4}"
        
        echo "✓ LLM Backend: openai"
        echo "✓ Model: $OPENAI_MODEL"
        echo ""
        ;;
        
    ollama)
        echo -e "${YELLOW}🤖 Configuring Ollama...${NC}\n"
        
        export LLM_BACKEND="openai_compatible"
        export LLM_BASE_URL="${LLM_BASE_URL:-http://localhost:11434/v1}"
        export LLM_MODEL="${LLM_MODEL:-llama2}"
        export LLM_API_KEY="ollama"
        
        echo "✓ LLM Backend: openai_compatible (Ollama)"
        echo "✓ Base URL: $LLM_BASE_URL"
        echo "✓ Model: $LLM_MODEL"
        echo ""
        
        # Check if Ollama is running
        if ! curl -s "$LLM_BASE_URL/models" > /dev/null 2>&1; then
            echo -e "${RED}Error: Ollama is not running at $LLM_BASE_URL${NC}"
            echo "Please start Ollama with: ollama serve"
            exit 1
        fi
        ;;
        
    *)
        echo -e "${RED}Error: Unknown LLM backend: $LLM_BACKEND${NC}"
        echo "Supported backends: qwen, claude, openai, ollama"
        exit 1
        ;;
esac

# Setup database
export DATABASE_URL="${DATABASE_URL:-sqlite::memory:}"
export RUST_LOG="${RUST_LOG:-ironclaw=debug}"

echo -e "${YELLOW}📦 Database Configuration${NC}"
echo "✓ Database URL: $DATABASE_URL"
echo ""

# Run tests
echo -e "${YELLOW}🧪 Running Integration Tests${NC}\n"

echo "Test 1: Basic job creation"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo test --test e2e_real_llm_job_creation test_create_job_via_chat_real_llm \
    --features libsql -- --nocapture --test-threads=1

echo ""
echo "Test 2: Multiple jobs"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo test --test e2e_real_llm_job_creation test_create_multiple_jobs_real_llm \
    --features libsql -- --nocapture --test-threads=1

echo ""
echo "Test 3: Job state transitions"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo test --test e2e_real_llm_job_creation test_job_state_transitions_real_llm \
    --features libsql -- --nocapture --test-threads=1

echo ""
echo "Test 4: Chat job creation"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo test --test e2e_real_llm_chat_job_creation test_chat_create_job_with_real_llm \
    --features libsql -- --nocapture --test-threads=1

echo ""
echo "Test 5: Chat job workflow"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cargo test --test e2e_real_llm_chat_job_creation test_chat_job_workflow_with_real_llm \
    --features libsql -- --nocapture --test-threads=1

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ✅ All tests completed successfully!                      ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}\n"
