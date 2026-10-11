---
name: thermos
description: "Launch both thermo-nuclear review subagents in parallel, then synthesize their findings. Use for thermos, double thermo review, or combined bug/security and code-quality branch audits."
disable-model-invocation: true
---

<!-- Adapted for Claude Code from github.com/cursor/plugins (thermos @ dc41543), MIT, see ./LICENSE. -->

# Thermos

Run the two thermo review passes as background subagents in parallel, then synthesize their results.

## Workflow

1. Determine the review scope from the user request, PR, current branch, or relevant changed files. The default base branch in this repo is `master`. If the branch has no diff against its base and the user asked for a whole-codebase review, scope the review to the requested directories instead and say so in the subagent prompts.
2. Gather the diff (`git diff master...HEAD`) and any file/context excerpts needed for reviewers to evaluate the change without guessing.
3. Launch both subagents in the same message with the Agent tool:
   - `subagent_type: "thermo-nuclear-review-subagent"` for bugs, breakages, security, devex regressions, feature-flag leaks, and other branch-audit risks.
   - `subagent_type: "thermo-nuclear-code-quality-review-subagent"` for maintainability, structure, file-size growth, spaghetti, abstractions, and codebase-health risks.
   If those agent types are not registered yet (agents added mid-session load on the next session), use `subagent_type: "general-purpose"` and tell it to read and follow the matching file in `.claude/agents/`.
4. Pass each subagent the same scoped diff/file context and ask it to return prioritized findings with file references and evidence.
5. After both finish, synthesize the results with findings first, deduplicated across reviewers. Weight overlapping findings more heavily, resolve disagreements with your own judgment, and keep summaries brief.

If individual background summaries are already visible to the user, do not restate them wholesale. Surface the unified verdict, the highest-signal findings, and any remaining uncertainty.
