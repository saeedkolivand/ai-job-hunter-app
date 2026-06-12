---
description: Refactor code to meet quality standards (smallest safe diff), then typecheck + test
argument-hint: [package-or-path]
---

Load the `token-efficiency` + `code-quality` skills. Use the code-quality-author subagent to fix quality issues in $ARGUMENTS (current diff if empty). Apply the smallest behavior-preserving changes, then run typecheck and tests.
