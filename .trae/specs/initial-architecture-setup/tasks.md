# Tasks

- [ ] Task 1: Initialize Project Structure
  - [ ] Create `admin-backend/`, `client/`, `execution-env/` directories.
  - [ ] Initialize `admin-backend/` as a submodule (create repo, add submodule).

- [ ] Task 2: Setup Admin Backend Structure (in submodule)
  - [ ] Create `sec-pipeline/` (SCA, Static, Signer).
  - [ ] Create `iam/` (LDAP/SSO/UKey).
  - [ ] Create `store-svr/` (Skills Store).
  - [ ] Create `policy-engine/` (RBAC, DLP).
  - [ ] Create `audit-center/` (LogSync, Reconcile, LogVault).

- [ ] Task 3: Setup Client Terminal Structure (Tauri)
  - [ ] Create `client/src-tauri/` (Rust backend).
  - [ ] Create `client/src/` (Frontend UI).
  - [ ] Create UI modules: `login/`, `chat/`, `store/`, `approval/`.
  - [ ] Create Security Kernel modules: `validator/`, `dlp-engine/`, `wasm-box/`, `audit-proxy/`, `sync-manager/`.

- [ ] Task 4: Setup Execution Environment Structure
  - [ ] Create `execution-env/mcp-bridge/`.
  - [ ] Create `execution-env/llm-gateway/` (ModelProxy, LocalLLM).
  - [ ] Create `execution-env/openclaw-skills/`.

# Task Dependencies
- Task 2 depends on Task 1.
- Task 3, 4 can run in parallel with Task 2.
