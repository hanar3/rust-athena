# Rodecomp emulation roadmap

The baseline is registration, login, character creation and reload, map transfer, Prontera load, and single-player walking, all verified in Rodecomp. The first release target proposed here is a playable loop for `PACKETVER=20260219`. The full list of features and mechanics to emulate is still open.

GitHub Issues in `hanar3/rust-athena` will track implementation work. Keep design decisions in the repository and link them from issues. Keep verification evidence with the issue or its implementing PR. Use rAthena to establish observable behavior and wire compatibility. Use Rust Athena’s own process and module design.

## Proposed sequence

| Milestone | Playable outcome | Main prerequisites |
| --- | --- | --- |
| Shared map | Two clients see each other enter, move, leave and reconnect in Prontera. The server owns their live positions. | Current login-to-walk flow and map sessions |
| Persistent travel | A character enters a second map, disconnects and resumes at the saved position. | Map sessions, save lifecycle, map routing |
| First gameplay loop | Fight one monster, gain experience, pick up a drop, equip it, and see the result after reconnecting. | Shared actors, character stats, combat timing, inventory persistence |
| First content loop | Talk to one NPC, use a shop or warp, and finish a quest that survives reconnect. | Map routing, items and currency, character persistence |
| Broader parity | Cover the agreed jobs, skills, content, and multiplayer systems against an explicit feature inventory. | Earlier loops and a confirmed parity target |

Inventory the broader systems by observable player flows. Start with account and character lifecycle, map changes and warps, combat and statuses, equipment and item lifecycle, NPC scripting and quests, party and guild play, trading and mail, and game modes and events. Give each flow an evidence status such as verified, partial, missing, or unknown. Add operational work such as recovery, migrations, and observability where it blocks those flows. Revisit the inventory after every playable milestone. This keeps discovery open as more client behavior becomes visible.

## Issue and PR evidence

Every implementation issue should include:

- **Starting point and outcome.** Describe what happens today and what the player or operator should be able to do afterward.
- **Implementation steps and dependencies.** Name affected modules, prerequisites, expected state changes, failure behavior, and explicit scope limits.
- **Reference files.** Link to Rust Athena, rAthena, and Rodecomp. Pin the revisions used when filing and record the packet version. Include the relevant function or packet names.
- **Acceptance evidence.** Include deterministic tests and, when the outcome is visible, a Rodecomp procedure with expected results. Record the client build, Rust Athena commit, commands, and actual results with the issue or PR. Use synthetic accounts and redact credentials and handoff codes.
- **Decision trail.** Link any decision record and capture open questions before changing the plan. Link follow-up issues for behavior discovered while implementing.

## First implementation issues

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

## Parity decision

Choose what will count as full emulation before treating this as a finite release plan. Protocol coverage measures which requests and replies work. Gameplay parity also requires matching rules, content and persistence across player scenarios. The milestone sequence supports either target while the feature inventory is built.
