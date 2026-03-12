# Initial Architecture Setup Spec

## Why
To implement the governance architecture defined in `governance/architecture.md`, separating concerns into distinct modules for security, maintainability, and scalability. The Admin Backend requires strict separation (separate repository) for security auditing and access control.

## What Changes
- Create a modular project structure reflecting the architecture diagram.
- **Admin Backend**: Initialize as a git submodule in `admin-backend/`.
- **Client Terminal**: Set up a Tauri application structure in `client/`.
- **Execution Environment**: Create a service structure in `execution-env/`.
- **Security Kernel**: Implement core Rust modules in `client/src-tauri/src/kernel/`.

## Impact
- **New Directories**: `client/`, `admin-backend/`, `execution-env/`.
- **New Configuration**: `.gitmodules` for the admin backend.
- **Affected Specs**: None (initial setup).

## ADDED Requirements

### Requirement: Admin Backend Submodule
The system SHALL organize the Admin Backend code in a separate git repository and include it as a submodule at `admin-backend/`.
- **Components**:
    - `audit-center/`: LogSync, Reconcile, LogVault.
    - `sec-pipeline/`: SCA, Static, Signer.
    - `iam/`: Identity Access Management.
    - `store-svr/`: Skills Store Server.
    - `policy-engine/`: Policy management.

### Requirement: Client Terminal Structure
The system SHALL organize the Client Terminal as a Tauri application at `client/`.
- **UI Layer (`client/src/`)**:
    - `login/`: LoginUI.
    - `chat/`: Chat interface.
    - `store/`: StoreUI.
    - `approval/`: ApprovalUI.
- **Security Kernel (`client/src-tauri/src/kernel/`)**:
    - `validator/`: SM2 Signature Validator.
    - `dlp-engine/`: Data Loss Prevention Engine.
    - `wasm-box/`: WASM Runtime Sandbox.
    - `audit-proxy/`: Offline Audit Proxy.
    - `sync-manager/`: Resumable Upload Manager.

### Requirement: Execution Environment Structure
The system SHALL organize the Execution Environment at `execution-env/`.
- **Components**:
    - `mcp-bridge/`: MCP Protocol Bridge.
    - `llm-gateway/`: Model Proxy and Local LLM interface.
    - `openclaw-skills/`: Ecosystem plugins.
