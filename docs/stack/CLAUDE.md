# CLAUDE.md — The Bear Cave

## What This Is

A unified media infrastructure stack combining the active media services from media-stack and metacacharr into a single, cohesive Docker Compose deployment. 8 containers, one `docker compose up -d`. Retired services remain documented in `docs/services/lifecycle.md` and preserved under `archive/`.

## How to Work

### Safety First

- Verify `.gitignore` before committing if `.env` or configuration files are modified.
- Never expose secrets. Check for leaked API keys, passwords, or tokens.
- Get explicit confirmation before executing destructive operations.
- State actions clearly and wait for verification before touching production environments.

### Search Before Building

Follow three layers in order:
1. Tried-and-true standard libraries
2. New-and-popular libraries with proven traction
3. First-principles custom code (requires documented justification)

### Test Before Shipping

- Ship a test suite AND an eval suite in the same commit for every feature or bug fix.
- Run health checks against all services after any compose file changes.
- Verify the full pipeline: request → download → serve → play.

### Documentation

- Every service must have a README with purpose, configuration, and troubleshooting.
- Architecture diagrams must be both ASCII and Mermaid format.
- Quick Start guide must be copy-pasteable for a fresh installation.

### Completion Status Protocol

Report exactly one status at the end of every task:
- **DONE:** All steps finished, tests passing, evidence provided, ready to merge.
- **DONE_WITH_CONCERNS:** Completed, but with specific issues listed.
- **BLOCKED:** Cannot proceed. State the blocker and attempted solutions.
- **NEEDS_CONTEXT:** Missing information. State exactly what is required.

## The Two Machine Spaces

- **Latent Space (LLM Work):** Judgment, pattern matching, creativity, open-ended analysis. High variability.
- **Deterministic Space (Code):** Precision, reproducibility, speed, zero cost per run, testable.
- **The Rule:** If a task is deterministic, write a script. Let the script constrain the LLM to eliminate failure paths.

## Context Window is the Lever

Treat the context window as a deliberate, curated input. Load the spec, contract, relevant files, and concrete examples. Exclude noise.

## How to Talk

- Be direct, short, and concrete.
- Reference specific file names, function names, and line numbers.
- State broken elements plainly.
- End all responses with the next immediate action.

## After Every Task

1. Stage and commit work with a clear message.
2. State exactly which system or service needs to be restarted to apply changes.

## Confusion Protocol

For high-stakes ambiguity, STOP. State the ambiguity in one sentence, present 2-3 options with concrete trade-offs, and wait for confirmation.

## Prompt Rewriting

Before acting on any user prompt, evaluate whether rewriting it for clarity, typo correction, or actionability would meaningfully improve it. **Only rewrite when there is a real improvement to make** — typos to fix, ambiguous references to resolve, or vague intent to specify. If the original prompt is already clear and actionable, proceed without rewriting and without showing a rewrite. When you do rewrite, display the rewritten version with the prefix "**Rewritten prompt:**" and wait for approval before proceeding.

**Full rules** (skip categories, mode behavior, rewriting style): see `/skill prompter` SKILL.md. Do NOT duplicate the skip list here; the skill file is the source of truth.
