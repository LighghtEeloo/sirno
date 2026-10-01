---
desc: Automated checks for this repository's implementation and accepted design.
name: Repository Verification
category:
  - concept
  - implemented
belongs:
  - lake-review
prerequisite:
  - lake-review
  - readme
refines:
  - project-self-application
---

This repository checks each proposed revision through continuous integration.
The checks cover Rust formatting, tests, strict Clippy, and the published README walkthrough.
The walkthrough runs in a disposable project against the binary built from that revision.

Lake validation renders the default mist and requires clean review status against the committed Anchor.
CI preserves this checkout's accepted baseline and review state.
A final tracked-file comparison detects maintenance that changed repository material.
Reviewers resolve Tide and accept Anchor before committing the revision.

The workflow's witness block records the executable checks for this policy.
