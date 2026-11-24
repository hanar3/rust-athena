# Rodecomp emulation roadmap

The baseline is registration, login, character creation and reload, map transfer, Prontera load, and single-player walking, all verified in Rodecomp. The first major milestone is complete client-facing packet coverage for `PACKETVER=20260219`, with persistence wherever those exchanges require it.

Use rAthena to establish observable behavior and wire compatibility. Use Rust Athena’s own process and module design.

## How work moves from issue to PR

1. **File the issue first.** Put each deliverable in [GitHub Issues](https://github.com/hanar3/rust-athena/issues). Write it for a contributor who knows Rust, Ragnarok, and networking and is new to this repository. Include the starting point, the result to build, the files to read, the steps, any dependencies, and how to verify it.
2. **Pick up an unblocked issue.** Use the issue as the implementation brief. Record discoveries and scope changes on the issue. If a discovery calls for separate work, file and link a follow-up.
3. **Open a PR linked to the issue.** Use `Closes #N` when the PR implements the full issue. Explain the decisions that shape the implementation, list what was verified and how, and identify any live Rodecomp check still pending. Record durable protocol findings and design decisions in the repository so future contributors can find them after the PR merges.

## Milestone 1 · Packet coverage and persistence

Use the [versioned packet inventory](protocol/README.md) to track client-facing packets for `PACKETVER=20260219` across login, character, and map connections, in both directions. Reconcile Rodecomp’s packet definitions, send sites, and receive handlers with rAthena’s packet tables and behavior for the same version. Record the opcode, direction, connection phase, framing, owning flow, reference revisions, and verification status. Track disagreements and unknowns until resolved.

Use that inventory to file implementation issues by exchange or player flow. Start from the verified login-to-walk path and extend through character lifecycle, map entry and travel, actor updates, inventory and equipment, combat and statuses, NPC interactions, and social and commerce exchanges. A flow may require enough server behavior to produce correct replies and state changes. Full gameplay rules and content come later.

### Completion criteria

- Every in-scope packet is accounted for in the inventory. Implement requests, replies, and server-initiated notifications in the correct connection phase. Verify intentionally ignored packets from reference behavior. Document reserved or obsolete entries with evidence for their classification.
- Each exchange has framing and behavior checks. Verify client-visible flows in Rodecomp, with pinned reference revisions and links to the implementing issue or PR. A packet that merely decodes does not count as covered when the exchange also requires a reply or state change.
- Durable state required by those exchanges survives reconnects and restarts. Document what is saved, which process owns it, when it is written, and how to reproduce the save-and-reload check. Examples include accounts, characters, location, and inventory as their flows are implemented.
- No packet or persistence entry is left with an unknown or partial status at milestone completion. Track any discrepancy between the two references and record the compatibility decision.

## Later milestones · Gameplay and content parity

After the packet and persistence milestone, set separate acceptance criteria for matching gameplay rules, jobs and skills, maps and NPC scripts, quests, game modes, and content. Use the verified protocol inventory as the starting point for those plans.

## Issue and PR evidence

Every implementation issue should include:

- **Starting point and outcome.** Describe what happens today and what the player or operator should be able to do afterward.
- **Implementation steps and dependencies.** Name affected modules, prerequisites, expected state changes, failure behavior, and explicit scope limits.
- **Reference files.** Link to Rust Athena, rAthena, and Rodecomp. Pin the revisions used when filing and record the packet version. Include the relevant function or packet names.
- **Acceptance evidence.** Include deterministic tests and, when the outcome is visible, a Rodecomp procedure with expected results. Record the client build, Rust Athena commit, commands, and actual results with the issue or PR. Use synthetic accounts and redact credentials and handoff codes.
- **Decision trail.** Link any decision record and capture open questions before changing the plan. Link follow-up issues for behavior discovered while implementing.

## First implementation issues

[Issue #7](https://github.com/hanar3/rust-athena/issues/7) establishes the client packet inventory and current coverage baseline.

### [#1 Give admitted characters map-owned session state](https://github.com/hanar3/rust-athena/issues/1)

**Starting point.** The map connection handler owns the character’s tile and pending arrival in local variables. Other map sessions cannot query that state.

**Steps.** Define when an admitted character becomes active, how the map role identifies it, and what happens when a second connection presents the same character. Move presence and authoritative position into map-owned runtime state. Keep socket handling responsible for packet I/O. Update position as movement completes and remove the correct session on disconnect or timeout. Keep the current single-client wire behavior and fixed spawn while this ticket changes ownership. Leave visibility and save writes to separate issues.

**Done when.** Tests cover admission, rejection or replacement under the documented duplicate-session policy, arrival, and cleanup. A disconnect from an old connection cannot remove a newer session. A simulated clock verifies that the authoritative position advances without a second click. In Rodecomp, a saved character can still enter Prontera and walk after the change.

**Files to read.** Rust Athena [map role](../core/src/map_server.rs), [listener](../core/src/listener.rs), [map transfer persistence](../db/src/lib.rs), [terrain](../core/src/terrain.rs). rAthena [map sessions and actor lookup](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/map.cpp), [unit movement](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/unit.cpp). Rodecomp [actor and movement packet handlers](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameModePackets.cpp).

### [#2 Show nearby players entering, walking, and leaving Prontera](https://github.com/hanar3/rust-athena/issues/2)

**Starting point.** A walking player receives its own movement acknowledgement. Other sessions receive no actor lifecycle events.

**Depends on.** [#1](https://github.com/hanar3/rust-athena/issues/1).

**Steps.** Compare a two-client rAthena session with the Rodecomp actor entry, movement, and removal handlers. Document the packet layouts, actor ID, appearance fields, and visibility transitions observed in the target revision. Send the newcomer the nearby actors after map load and announce the newcomer to those actors. Keep the existing walk confirmation for the originating client and send observer movement to visible peers. On range exit or disconnect, remove the actor from viewers. Keep NPCs, combat and cross-map routing outside this ticket.

**Done when.** Two Rodecomp clients see each other in range, track one another walking, lose visibility when separated, and remove the other character on disconnect. Reconnecting shows one actor rather than a duplicate. Tests cover entry, move, range exit, and cleanup using two distinct sessions and the actual encoded packets.

**Files to read.** Rust Athena [map role](../core/src/map_server.rs) and [wire module](../core/src/protocol/v20260219.rs). rAthena [actor entry and visibility packets](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/clif.cpp), [movement lifecycle](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/unit.cpp). Rodecomp [actor entry and removal handlers](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameModePackets.cpp), [actor initialization](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/EntityInit.cpp).

[Issue #5](https://github.com/hanar3/rust-athena/issues/5) starts persistent travel by saving and restoring the last reached tile in Prontera.
