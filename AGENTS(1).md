# Agent Guide

Read [PROJECT_PLAN.md](PROJECT_PLAN.md) before working. It defines the project scope, references, acceptance criteria, and current status. Inspect the code and reconcile that status with reality.

Build a new Rust engine that imports Super Mario 64 content from a user-supplied ROM. Bob-omb Battlefield is the first playable target; shared course, area, act, behavior, and warp systems must leave room for the complete game.

Preserve original movement, collision, and timing. Keep simulation at the supported reference version's cadence and interpolate presentation independently. Establish per-tick comparisons before claiming fidelity. Graphics settings must not modify authoritative gameplay state. Intentional gameplay changes require explicit optional modes.

Reference or adapt community work instead of guessing formats or reinventing established algorithms. Check source terms, pin revisions, preserve notices, and record provenance. Keep ROMs and ROM-derived assets out of version control. Temporary C integrations must have documented boundaries and a replacement plan; the intended engine and game runtime are Rust.

Choose implementation details within these goals. Deliver small, usable increments; test changes meaningfully and update PROJECT_PLAN.md, build/run instructions, and provenance records as needed. Report what works, what was tested, what remains missing, and the next useful task. Distinguish level exploration, mission completion, and complete-course support.
