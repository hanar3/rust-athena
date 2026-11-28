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

## Movement packet layout

`NotifyPlayerMove` encodes the `0087` opcode and the walk-start tick as little-endian fields, followed by a six-byte `WalkRoute`. The route encodes source X/Y and destination X/Y as four 10-bit values, most-significant bit first, followed by two four-bit sub-tile offsets set to 8. Moving actor entry (`09fd`) shares the route encoder. Fixtures check both packet paths, including coordinates above 511.

Deku derives the wire encoding for both types. The field attributes declare byte order and bit widths, and a shared writer sends the result into the existing fixed-size buffers. The map supplies coordinates within 0–1023 and the fixtures exercise the same write paths used during play.

[Issue #15](https://github.com/hanar3/rust-athena/issues/15) · [rAthena movement encoder](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/clif.cpp#L2052) · [Rodecomp movement decoder](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameModePackets.cpp#L23653) · [Deku attributes](https://docs.rs/deku/0.20.3/deku/attributes/index.html)

## Map facing

Rodecomp sends `0361` (5) to update head and body direction. The map owner validates both values and sends `009c` (9) to visible observers. Actor entry packets include the current direction for newcomers. Facing is transient state. Codec, visibility, and two-client TCP checks passed. Live Rodecomp verification is pending.

[Issue #11](https://github.com/hanar3/rust-athena/issues/11) · [rAthena direction handler](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/clif.cpp#L11581) · [Rodecomp direction decoder](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameModePackets.cpp#L23842)

## Map quit

| Exchange | Client request | Server reply | Behavior and evidence |
| --- | --- | --- | --- |
| Quit from the map | `018a` (4) | `018b` (4), result 0 allows quit and result 1 denies it | Rodecomp sends the request from its Quit action and closes after a successful reply. Rust Athena accepts quit after map load, saves the last reached tile before sending success, and denies the request before load. Codec and local TCP/MariaDB checks passed. A live Rodecomp check is pending. |

[Issue #9](https://github.com/hanar3/rust-athena/issues/9) · [rAthena quit handling](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/clif.cpp#L11435) · [Rodecomp quit request](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameMode.cpp#L1831) · [Rodecomp acknowledgement handling](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameModePackets.cpp#L26628)

## Return to character select

Rodecomp sends `CZ_RESTART` (`00b2`, three bytes) with type 1. On `ZC_RESTART_ACK` (`00b3`, three bytes) with type 1, it closes the map socket and reconnects to the advertised character endpoint using the original account ID, both login IDs, and account sex. Rust Athena saves the last reached tile, creates a 60-second, single-use character handoff, and releases the map session before the writer sends success. The server waits for the client to close. Rodecomp processes transport EOF before dispatching buffered packets, so closing immediately after the acknowledgement can cause a disconnect. A type 1 request before map load or from a legacy transfer receives `018b` with result 1. Other request types are consumed. Type 0 is the respawn path and still needs an implementation.

The codec and local TCP/MariaDB checks covered switching between two characters, returning to the saved tile, replay refusal, old transfer rows, and a competing login. Live Rodecomp verification is pending. Apply the map return credentials migration, start the three roles with Prontera walkability enabled, walk to a new tile, and choose Character Select from the Esc menu. Select a second character, repeat the return, and select the first character to verify its saved position.

[Issue #13](https://github.com/hanar3/rust-athena/issues/13) · [rAthena restart handling](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/clif.cpp#L11832) · [Rodecomp acknowledgement](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameModePackets.cpp#L24491) · [Rodecomp character reconnect](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/LoginMode.cpp#L3656)

## Reconciliation workflow

Start with one exchange, then trace its client send and receive paths, rAthena's active packet registration and handler, and our implementation. Record framing, state transitions, refusal behavior, and which values must survive restarts. File a contributor-ready issue for behavior that is missing or partial, and link the implementing PR and verification result back to the inventory notes. Preserve reference disagreements until they are resolved by target-version behavior. An opcode with no reviewed exchange is unresolved, even when both references name it.
