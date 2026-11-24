# PACKETVER=20260219 packet inventory

The [Rodecomp index](rodecomp-20260219.csv) is a discovery inventory. It covers the union of the client's named packet IDs and frame registrations, including legacy names and 106 unnamed entries. The pinned union has 1,820 unique opcodes. A row establishes that an ID appears in reference data. To establish a supported exchange, trace a sender or receiver for the target client build, match the server's packet layout and state transitions, and verify the result.

## Reproduce the index

Run this from the Rust Athena root with Python 3, Git, and a Rodecomp checkout containing the pinned commit. The script reads Git objects, so uncommitted changes in Rodecomp do not affect the output.

```sh
python3 tools/inventory_rodecomp.py /path/to/rodecomp > /tmp/rodecomp-20260219.csv
diff -u docs/protocol/rodecomp-20260219.csv /tmp/rodecomp-20260219.csv
```

- `aliases` preserves all names associated with an opcode. `prefix_hints` records the likely direction and connection role where the prefix has a clear meaning. Inspect send sites and receive dispatch before treating a hint as confirmed.
- `first_registration_framing` follows Rodecomp's first-insertion lookup. A variable registration's value is Rodecomp's raw variable parameter. It is neither the observed wire length nor a substitute for checking the packet structure.
- `registrations_record_variable_length` preserves each raw tuple in source order. Names without a registration and registrations without a name remain in the inventory. IDs shared across revisions or connection phases need separate exchange records during reconciliation.

Rodecomp revision `ce6d40783aa3a98043a232c28678ae32f9368dd5`. [Packet IDs](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Network/Packets/PacketId.h) · [Registrations](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Network/Packets/PacketRegistrations.inc) · [Lookup implementation](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Network/Packets/PacketLengths.cpp)

rAthena comparison revision `226bd83d39584bc91b6bf133f40bce95aa1f178c`. Apply compile-time conditions and packet-table overrides when reconciling a flow. A textual match in a legacy version block does not establish behavior for this date. [Shared layouts](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/common/packets.hpp) · [Login dispatcher](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/login/loginclif.cpp) · [Map packet table](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/clif_packetdb.hpp) · [Map layouts](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/packets_struct.hpp)

## Rust Athena coverage baseline

Directions are relative to the server. Multiple opcodes on one line belong to the same exchange. Wire sizes describe the current Rust Athena implementation. See the [codec](../../core/src/protocol/v20260219.rs) and [architecture evidence](../architecture.md) for existing tests and client observations.

| Connection and flow | Inbound | Outbound | Status and persistence |
| --- | --- | --- | --- |
| Login credentials | `0064` (55) | `0ac4` (variable, 224 for one endpoint), `083e` (26) | Client-confirmed success. Local TCP tested wrong-password response. Account lookup and expiring login handoff persist in MariaDB. The catalog contains other login variants to reconcile. |
| Login digest before credentials | `0204` (18) | — | Partial. Consumed when present, without digest validation. |
| Character admission and initial roster | `0065` (17) | Raw 4-byte account ID then `082d` (29), `006b` (variable), `09a0` (6), `020d` (variable), `08b9` (12), or `006c` (3) after the account ID on refusal | Client-confirmed roster and local TCP admission and replay checks. Redeems the login handoff and loads persisted characters. Block list and PIN currently represent empty/disabled states. |
| Character page and keepalive | `09a1` (2), `0187` (6) | `0b72` (variable) for the page | Client-confirmed roster. Fixture checks one character. Special three-record terminal page behavior has a unit test. |
| Character creation | `0a39` (36) | `0b6f` (177), `006e` (3) | Client-confirmed creation and reload. Saved character row. Current creation rules cover selected jobs and validated appearance fields. |
| Character selection | `0066` (3) | `0ac5` (156), `02ca` (3) | Client-confirmed Prontera route. Issues a durable, expiring transfer. Other destinations are missing. |
| Map admission and load | `0436` (23), `007d` (2), `0447` (2) | `0283` (6), `02eb` (13), `0b1b` (2), `0074` (3) | Client-confirmed entry and load. Redeems transfer and uses persisted position when available. Confirm the target framing from Rodecomp before relying on rAthena's historical `0436` entries, which include a 19-byte variant. |
| Map connection maintenance | `0360` (6), `0b1c` (2) | `007f` (6), `0b1d` (2) | Local TCP verified. Uses the process clock. |
| Map startup requests | `01c0` (2), `08c9` (2) | — | Consumed. The pinned rAthena table specifies two bytes for both. Rust Athena currently has an empty catalog. |
| Map movement and actor visibility | `035f` (5) | `0087` (12), `09ff` and `09fd` (variable), `0080` (7) | Client-confirmed self movement. Two-client TCP tested appearance and visibility. Map state is in memory. Position writes on disconnect. |

## Reconciliation workflow

Start with one exchange, then trace its client send and receive paths, rAthena's active packet registration and handler, and our implementation. Record framing, state transitions, refusal behavior, and which values must survive restarts. File a contributor-ready issue for behavior that is missing or partial, and link the implementing PR and verification result back to the inventory notes. Preserve reference disagreements until they are resolved by target-version behavior. An opcode with no reviewed exchange is unresolved, even when both references name it.
