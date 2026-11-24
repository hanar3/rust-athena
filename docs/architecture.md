# Server architecture

## Process responsibilities

| Process | Owns | Persistence |
| --- | --- | --- |
| Login | Credential verification and the advertised character-server endpoint | Accounts and pending login handoffs |
| Character | Admission, roster loading, character creation, and selection | Login handoff redemption and character rows |
| Map | Admission to the configured map | Single-use map transfers |

All three running roles use a bounded blocking TCP listener with read and write timeouts. The role handlers exchange typed values with the client protocol module. They never address packet offsets themselves. The database crate owns SQL, connections and persistence constraints. Registration is currently a local CLI command.

Login issues an expiring handoff with two opaque login IDs. The character process admits a connection only when it deletes the matching, unexpired row. That single SQL operation makes concurrent redemption atomic across processes. A new login replaces any pending handoff for the same account. A unique account-and-slot key and a unique character-name key enforce creation constraints across connections. The processes share the database for this first deployment. A separate internal network protocol becomes useful when deployment requires independent data ownership or remote map coordination.

## Wire revisions and findings

`core/src/protocol/v20260219.rs` owns the supported client framing and layouts. Each listener starts with one known revision. A future revision should get its own layout and byte fixtures, and reuse account and character rules. First determine how that client identifies its revision, including whether separate listener configuration is needed.

- TCP reads use exact packet lengths. In particular, the optional login digest packet and the credential packet may arrive together or separately. Replies use complete writes.
- Character admission sends the raw account ID and then the slot, roster, page-notice, block-list, and PIN packets. The page reply uses the same 175-byte record format as the initial roster and character creation acknowledgement.
- Rodecomp finalizes paging when a reply contains a count other than three. rAthena sends a header-only follow-up when the count is exactly three. The checked-in one-record capture remains a golden test. [rAthena character exchange](https://github.com/rathena/rathena/blob/master/src/char/char_clif.cpp)
- The configured address in the login reply is the client-visible destination. The IP is written as IPv4 octets and the port is little-endian. The listener bind is configured independently. [rAthena login exchange](https://github.com/rathena/rathena/blob/master/src/login/loginclif.cpp)

## Map server in increments

rAthena's character process validates the account's requested slot and resolves the character's saved map against available map processes. It sends a destination packet, then the client opens a new socket. The map process decodes the connection request and asks the character process to authenticate it before continuing entry. Map loading and the client's load-complete acknowledgement are later steps in the exchange. [Character routing](https://github.com/rathena/rathena/blob/master/src/char/char_clif.cpp) · [Map connection and load acknowledgement](https://github.com/rathena/rathena/blob/master/src/map/clif.cpp) · [Map-to-character authentication](https://github.com/rathena/rathena/blob/master/src/map/chrif.cpp)

1. **Persistent account and character flow.** Verified in Rodecomp through registration, character creation, and reload after reconnecting.
2. **One map destination and admission.** The character process routes `prontera.gat` to the configured endpoint and sends `0x0ac5`. The client opens a new connection with `0x0436`. The map process atomically redeems the transfer and sends `0x0283`. A parsed request with an invalid transfer gets `0x0074`. Verified in Rodecomp. [Server destination packet](https://github.com/rathena/rathena/blob/master/src/common/packets.hpp) · [Rodecomp map transition](https://github.com/rodecomp/rodecomp/blob/main/Source/Client/Mode/LoginMode.cpp)
3. **Enter one map.** The map process sends `0x02eb` with a default spawn at `(156, 191)` and waits for the client’s load-complete packet (`0x007d`). It confirms the load with `0x0b1b` and handles Rodecomp’s time sync and ping requests. Verified in Rodecomp after matching rAthena’s destination map-name field.

The transfer expires after 60 seconds and is stored separately from the consumed login handoff. It binds the login auth code, selected character ID, owning account, character sex, and destination map. Issuing a new transfer replaces the pending transfer for that account. Multiple map processes serving the same map, health-based routing, and active character sessions are later milestones. The destination packet sends `prontera.gat` in a NUL-padded 16-byte field, matching rAthena. An earlier basename-only packet reached the client loading screen, where world resource lookup failed. [rAthena map-name encoding](https://github.com/rathena/rathena/blob/master/src/common/mapindex.cpp)

## Single-player movement

As Rodecomp enters the world, it can send two-byte requests for remaining playtime (`0x01c0`) and the cash-shop catalog (`0x08c9`). The map process consumes both without replying. rAthena also sends no catalog packets when all shop tabs are empty. [Packet table](https://github.com/rathena/rathena/blob/master/src/map/clif_packetdb.hpp) · [Catalog handling](https://github.com/rathena/rathena/blob/master/src/map/clif.cpp#L16123)

The client sends a packed destination in `0x035f`. Once the map is loaded, the process checks that destination against Prontera’s walkability data and searches up to 14 steps, including diagonals where both adjacent cardinal tiles are open. The map process sends `0x0087` with the source, destination and server tick for an accepted walk. The map process owns each character’s position and pending arrival. A connection holds an exclusive session from transfer redemption until disconnect. A duplicate map connection for the same account is refused while the original session is alive. On this map, the account ID is the actor ID on the wire and the character ID identifies the saved character. The character becomes active after the client’s map-loaded packet. A dedicated map thread owns live state and receives commands over a bounded MPSC queue. It advances movement on a 25 ms tick even under command traffic. Connection threads handle database and socket I/O. A full command queue applies backpressure to the sender. Clicks during a walk are ignored.

The map role can load Prontera’s terrain from a local rAthena map cache. The protocol module owns the packet layout, while the terrain module owns cache decoding and route validation. The map confirms a player’s movement with the existing client protocol. [rAthena walk acknowledgement](https://github.com/rathena/rathena/blob/master/src/map/clif.cpp#L1933) · [rAthena terrain cell types](https://github.com/rathena/rathena/blob/master/src/map/map.cpp#L3076)

## Reconnecting in Prontera

The character table stores nullable `last_x` and `last_y` coordinates for the saved map. Once a connection has reserved the account’s map session, it reads the committed tile and sends it in the existing `0x02eb` acceptance packet. A missing position, a coordinate outside the wire range, or a blocked tile in the configured cache falls back to (156, 191). Without a cache, the map accepts saved coordinates in the wire range and movement stays disabled.

When the loaded connection ends, the map loop advances any elapsed walking steps, removes the actor from view and returns the last reached tile. The connection thread writes it to MariaDB while still holding the reservation. A connection that ends before loading leaves the stored coordinates intact. The explicit quit request (`018a`) uses the same save path. The server commits the position before queuing the success acknowledgement (`018b`) and closing the map socket. A quit request before load receives a refusal and leaves the save intact. This saves on disconnect while the map process is running. Periodic saves, process-crash recovery and cross-map travel are outside this milestone. [rAthena player save state](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/pc.cpp) · [Rodecomp map acceptance](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/LoginMode.cpp)

## Nearby players

The map tracks loaded players and updates their position as they reach successive tiles. Two players on this map see each other within 14 tiles along both axes. Load completion, movement across that range, and disconnect update the set of visible peers. Each connection has a bounded write queue. Map state enqueues packets in order and the connection’s writer handles socket I/O.

| Wire response | Recipient and contents |
| --- | --- |
| `0x09ff` | A player who gains sight of a stationary peer. Carries the peer’s account and character IDs, position, name, sex, job, hair and palette. |
| `0x09fd` | A player who gains sight of a walking peer, or who can already see the peer when they start walking. Carries appearance and route fields. |
| `0x0087` | The walking player. Confirms their own route. |
| `0x0080` | A player who loses sight of a peer at the range edge or on disconnect. |

The entry layouts follow rAthena’s idle and walking actor structs and Rodecomp’s `ReceiveActorStateEntryNamedBody` and `ReceiveActorMoveEntryBody` decoders. NPCs and map transfers are separate work. [rAthena actor structs](https://github.com/rathena/rathena/blob/226bd83d39584bc91b6bf133f40bce95aa1f178c/src/map/packets_struct.hpp) · [Rodecomp actor decoders](https://github.com/rodecomp/rodecomp/blob/ce6d40783aa3a98043a232c28678ae32f9368dd5/Source/Client/Mode/GameModePackets.cpp)

## Verification

- Protocol tests cover the captured character page, empty and three-record page termination, and character-create framing with a following packet.
- A disposable MariaDB 11 instance verified both migrations, registration, wrong-password refusal, login and character admission on separate processes, replay refusal, empty roster, character creation, duplicate-slot refusal, map-unavailable selection, and character reload after restarting the character process.
- The map packet test covers destination encoding, entry decoding, truncation, and the account acknowledgement. A three-process MariaDB check verified fragmented entry, mismatched claims, concurrent redemption, expiry, and a fresh transfer. Rodecomp confirmed the transfer and admission. The spawn packet and load handshake are covered by the protocol and three-process TCP tests. The live spawn was verified after restoring the full map name. Single-player movement has protocol and terrain regressions plus a local TCP check against real Prontera terrain. Single-player movement was verified in Rodecomp after handling both startup requests. [Roadmap and first issue drafts](roadmap.md)
