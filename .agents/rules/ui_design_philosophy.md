# Work-First UI Design Standard & System Prompt
# Standard: "Work-First, Scenario-Driven, Manual-Before-Automation"

---
trigger: always_on
description: Mandatory architectural and ergonomic guidelines for designing, building, or refactoring user interfaces across projects.
---

## 🏛️ The Core Doctrine
> **"A good UI is not about showing off colors, shiny gradients, and marketing boxes. A good UI is built for actual work: driven by real user scenarios, operating manual-first before automation, and prioritizing high information density and keyboard ergonomics."**

---

## ⚡ The 6 Foundational Tenets

### 1. Anti-SaaS-Fluff (High Information Density)
- **The Pitfall:** Giant cards, wide padding, low information density, and decorative boxes designed to look pretty in pitch decks rather than facilitate daily work.
- **The Standard:** Screen real estate is a precious high-throughput canvas. Use compact grids, clear visual hierarchy, monospace/code-friendly typography, and zero wasted whitespace.
- **Aesthetic:** Clean CRT / Terminal TUI (Ghostty, Alacritty, k9s, htop, lazygit) for developer and operator tools.

### 2. Scenario-First Architecture (Scenarios Sell The App)
- **The Pitfall:** Generic dashboards that display arbitrary counters and expect the user to figure out what to do.
- **The Standard:** Identify the exact questions, fears, and real-world problems the user faces:
  - *"Did an agent touch my .env or credentials?"*
  - *"What exact lines of code changed?"*
  - *"Did prompts leak to expensive cloud APIs?"*
  - *"Can I mathematically prove logs weren't tampered with?"*
  - *"Can I freeze this runaway process right now?"*
- Design the primary layout, columns, and filters directly around these questions.

### 3. Live Interactive Demonstrations (Prove It in the UI)
- **The Standard:** Don't just claim the app solves a problem—provide interactive test drives directly in the UI.
- Provide a dedicated **`[⚡ Live Scenario Demos]`** bar where the user can click a button or press a key to simulate the real-world danger, watch the gateway/app intercept it, and see the visual proof (e.g. unified diff colorization, red security alert, blast radius containment) with a clear explainer card.

### 4. Manual-First Ergonomics (Human-in-the-Loop before Blind Automation)
- **The Principle:** Having an AI model audit another AI agent creates a circular trust paradox. Users distrust black boxes and want to do things **manually** before trusting automation.
- **Mandatory Manual Controls:**
  1. **Manual Compound Filtering:** Filter by entity, verdict, target, or type on the fly without writing scripts.
  2. **Screen Freeze / Pause (<kbd>P</kbd>):** Pause incoming data streams so logs don't scroll away during investigation.
  3. **Step-Through Navigation (<kbd>j</kbd>/<kbd>k</kbd>):** Step event-by-event through history.
  4. **Interactive Sandbox / "What If?" Form (<kbd>M</kbd>):** Let the user manually test policy rules against a target path *before* letting an autonomous process run.
  5. **Manual Flagging / Pinning (<kbd>F</kbd>):** Bookmark suspicious events for post-mortem analysis.
  6. **Raw Forensic Export:** One-click export of filtered data to un-doctored JSON or CSV for tickets and PRs.

### 5. Keyboard-First Navigation (Vim / TUI Command Flow)
- Every primary action must have a single-key shortcut:
  - <kbd>j</kbd> / <kbd>k</kbd> or <kbd>↓</kbd> / <kbd>↑</kbd>: Move through items
  - <kbd>/</kbd>: Instant filter focus
  - <kbd>:</kbd>: Interactive command prompt
  - <kbd>P</kbd>: Pause / Resume stream
  - <kbd>D</kbd>: Jump to Diff view
  - <kbd>V</kbd>: Verify cryptographic integrity
  - <kbd>F</kbd>: Flag / Pin item
  - <kbd>M</kbd>: Open Manual Sandbox
  - <kbd>?</kbd>: Open keyboard cheatsheet & scenario walkthrough
- Mouse is an optional convenience, never a requirement for power operators.

### 6. Dual-Mode Pragmatism & Zero-Dependency Portability
- **Dual Mode:** When visual overviews are desirable for presentations, preserve them alongside the work interface (`/dashboard` vs `/console`), with a seamless 1-click toggle between them.
- **Zero Heavy Framework Bloat:** Keep operational interfaces lightweight, portable, and fast (sub-millisecond rendering). Prefer pure HTML/CSS/JS without heavy NPM build chains or multi-megabyte bundles unless strictly required by the user.

---

## 📋 The Pre-Implementation Checklist

Before declaring any UI complete, verify against this checklist:
- [ ] **Is information density maximized?** (Can the operator see 20-50 items without excessive scrolling?)
- [ ] **Are the core user scenarios explicitly addressed?**
- [ ] **Can the user test-drive the scenarios live via interactive buttons?**
- [ ] **Can the user pause/freeze the live data at will?**
- [ ] **Is there a manual "What if?" sandbox or evaluator form?**
- [ ] **Can the user flag/pin items and export raw evidence to JSON/CSV?**
- [ ] **Are single-key shortcuts implemented and documented in a `<kbd>?</kbd>` modal?**
- [ ] **Is the code notebook-friendly, modular, and adhering to Single Responsibility?**
