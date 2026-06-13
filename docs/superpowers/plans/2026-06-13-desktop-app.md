# Desktop App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a new root-level Electron desktop client in `desktop-app`.

**Architecture:** Use `electron-vite` to generate a standalone Electron + React + TypeScript app with separated main, preload, and renderer entry points. Keep it independent from deprecated `desktop-client` and `client-gui`, while allowing those directories to remain reference-only.

**Tech Stack:** Electron, electron-vite, React, TypeScript, npm.

---

### Task 1: Scaffold Desktop App

**Files:**
- Create: `desktop-app/package.json`
- Create: `desktop-app/electron.vite.config.ts`
- Create: `desktop-app/src/main/index.ts`
- Create: `desktop-app/src/preload/index.ts`
- Create: `desktop-app/src/renderer/src/App.tsx`

- [ ] **Step 1: Remove empty placeholder directories**

Run:

```bash
find desktop-app -type f -maxdepth 5 -print
rmdir desktop-app/src/main desktop-app/src desktop-app
```

Expected: the first command prints no files, and `rmdir` succeeds.

- [ ] **Step 2: Generate the app**

Run:

```bash
npm create @quick-start/electron@latest desktop-app -- --template react-ts --skip
```

Expected: `desktop-app` is created with Electron main, preload, and React renderer sources.

- [ ] **Step 3: Install dependencies**

Run:

```bash
cd desktop-app
npm install
```

Expected: dependencies install and `package-lock.json` is created.

- [ ] **Step 4: Verify static checks**

Run:

```bash
cd desktop-app
npm run typecheck
```

Expected: TypeScript checks pass.

- [ ] **Step 5: Verify build**

Run:

```bash
cd desktop-app
npm run build
```

Expected: Electron main/preload and renderer bundles are built successfully.
