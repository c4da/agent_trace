# Code Refactoring & Engineering Architecture Rules

## Abstract
This document defines the strict architectural standards, anti-monolith invariants, and coding style rules for all refactoring and implementation tasks across the workspace.

---

## 🏛️ Refactoring Directive & Engineering Principles

### 1. Strict Anti-"God Object / God System" Principle
- **No Monoliths**: Never generate monolithic 800+ or 1,000+ line files bundling multiple tools, interactions, or data schemas together.
- **Proactive Decomposition**: Decompose large files proactively into focused modules adhering strictly to the **Single Responsibility Principle (SRP)**.
- **Max Method Length**: Keep functions concise (<= 30 lines) with <= 3 arguments per function whenever possible.

### 2. Feature-per-File Architecture
- **Dedicated Cohesive Files**: Every distinct feature, modifier, handler, or tool must reside in its own dedicated, cohesive file (e.g. `extrude.rs`, `revolve.rs`, `fillet.rs`, `workgroups.rs`, `idle_worker.rs`).
- **Clean Gateway Coordinators**: Top-level `mod.rs` or `lib.rs` files act strictly as coordinators and clean API gateways (re-exporting public types, declaring submodules, and routing dispatch), never hoarding raw implementation logic.

### 3. Decoupled System Responsibilities
- **Decouple UI from Core Logic**: Never tangle UI callbacks or event listeners with raw business/topological mutations. Keep selection filtering, state mutations, command execution, and rendering decoupled.
- **Pure Coordinate Frames**: Child mesh/entity transforms must inherit parent transforms (`Transform::IDENTITY`), never arbitrary hardcoded offsets (e.g. `Transform::from_xyz(0.0, 0.5, 0.0)`).

### 4. Separation of Pure Mathematics from Frameworks
- **Pure Math Library Invariant**: All core geometric and mathematical algorithms live in pure standalone libraries/crates depending ONLY on lightweight foundations (`glam`, `serde`).
- **Zero Heavy Framework Dependencies in Math**: Zero dependencies on rendering engines, GPU assets, or UI toolkits in math modules. Guarantees sub-second compilation, portability, and instant headless testing.
- **Engine Framework Adapters**: Engine layers consume pure math crates and adapt results into ECS components, GPU buffers, or UI views.

### 5. Bulletproof Invariant-Driven Verification
- **Headless Invariant Tests**: Operations must never rely solely on visual inspection. Validate operations headlessly via rigorous invariants:
  - **Euler-Poincaré Characteristic**: $\chi = V - E + F = 2$ for closed 2-manifold polyhedra.
  - **Watertight 2-Manifoldness**: Every directed half-edge $(A \to B)$ paired with exactly one opposing $(B \to A)$.
  - **Positive Volume Divergence**: Divergence theorem volume $\sum \frac{v_0 \cdot (v_1 \times v_2)}{6} > 0$.
  - **Perimeter Seam Cancellation**: Shared internal edges cancel out; outer boundary vertices have degree $deg(v) = 2$.
- **Combinatorial Edge Case Coverage**: Author test suites covering concave polygons, multi-face connected patches, and non-coplanar ridges.

### 6. Zero-Token Economy & Cluster Delegation
- **Conserve External Tokens**:
  - **DeepSeek-R1 (PC-2)**: Ultra-fast GPU architecture reasoning, invariant sanity checks, and CoT verification.
  - **Qwen (PC-1)**: Primary code implementation, Rust/Python unit tests, boilerplate generation, and AST audits.
  - **Antigravity**: System architect, interface contract designer, and multi-file orchestrator.
- **Local VRAM Hygiene**: Issue unload requests (`POST /api/generate` with `{"keep_alive": 0}`) after heavy inferences to free memory.

### 7. Clean Diffs & Domain Hygiene
- **Pure Python SDKs**: Standard library only (`math`, `struct`, `socket`, `json`, `subprocess`). No mandatory heavy third-party wheels for core functionality.
- **Notebook-Friendly & High Test Coverage**: Keep methods short, write concise practical comments, and aim for >= 85% test coverage.
- **Clean Targeted Diffs**: Never overwrite credentials, `.env`, or VCS metadata.
