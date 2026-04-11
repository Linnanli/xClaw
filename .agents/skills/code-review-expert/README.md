# Code Review Expert

A comprehensive code review skill for AI agents. Performs structured reviews with a senior engineer lens, covering architecture, security, performance, and code quality.

## Installation

```bash
npx skills add sanyuan0704/sanyuan-skills --path skills/code-review-expert
```

## Features

- **SOLID Principles** - Detect SRP, OCP, LSP, ISP, DIP violations
- **Security Scan** - XSS, injection, SSRF, race conditions, auth gaps, secrets leakage
- **Performance** - N+1 queries, CPU hotspots, missing cache, memory issues
- **Error Handling** - Swallowed exceptions, async errors, missing boundaries
- **Boundary Conditions** - Null handling, empty collections, off-by-one, numeric limits
- **Removal Planning** - Identify dead code with safe deletion plans
- **Language References** - Load language-specific review checklists for React/TS, Vue, Rust, Python, Go, Java, and C/C++
- **Triggered Mandatory Priority Mapping** - Enforce fixed language review order only when risk triggers fire

## Usage

After installation, simply run:

```
/code-review-expert
```

The skill will automatically review your current git changes.

## Workflow

1. **Preflight** - Scope changes via `git diff`
2. **Language Pass** - Load relevant language checklist by changed files
3. **Priority Mapping (Triggered Mandatory)** - Apply fixed order only when trigger conditions match
4. **SOLID + Architecture** - Check design principles
5. **Removal Candidates** - Find dead/unused code
6. **Security Scan** - Vulnerability detection
7. **Code Quality** - Error handling, performance, boundaries
8. **Output** - Findings by severity (P0-P3)
9. **Confirmation** - Ask user before implementing fixes

## Severity Levels

| Level | Name | Action |
|-------|------|--------|
| P0 | Critical | Must block merge |
| P1 | High | Should fix before merge |
| P2 | Medium | Fix or create follow-up |
| P3 | Low | Optional improvement |

## Structure

```
code-review-expert/
├── SKILL.md                 # Main skill definition
├── agents/
│   └── agent.yaml           # Agent interface config
└── references/
    ├── solid-checklist.md   # SOLID smell prompts
    ├── security-checklist.md    # Security & reliability
    ├── code-quality-checklist.md # Error, perf, boundaries
    ├── removal-plan.md      # Deletion planning template
    ├── language-react-typescript.md # React/TS/JS behavior checks
    ├── language-vue.md      # Vue behavior checks
    ├── language-rust.md     # Rust concurrency and cancellation checks
    ├── language-python.md   # Python async/resource checks
    ├── language-go.md       # Go context/goroutine checks
    ├── language-java.md     # Java transaction/concurrency checks
    ├── language-c-cpp.md    # C/C++ memory/lifetime checks
    └── language-priority-rules.md # Trigger-based mandatory cross-language review order
```

## References

Each checklist provides detailed prompts and anti-patterns:

- **solid-checklist.md** - SOLID violations + common code smells
- **security-checklist.md** - OWASP risks, race conditions, crypto, supply chain
- **code-quality-checklist.md** - Error handling, caching, N+1, null safety
- **removal-plan.md** - Safe vs deferred deletion with rollback plans
- **language-react-typescript.md** - React lifecycle and TS boundary checks
- **language-vue.md** - Vue watcher/composable and cleanup checks
- **language-rust.md** - Rust async, cancellation, and state consistency checks
- **language-python.md** - Python async, resource, and exception-path checks
- **language-go.md** - Go context, goroutine, and idempotency checks
- **language-java.md** - Java transaction and concurrency checks
- **language-c-cpp.md** - C/C++ memory and threading checks
- **language-priority-rules.md** - Trigger-based mandatory review order for mixed-language diffs

## License

MIT
