# Server architecture

## Process responsibilities

| Process | Owns | Persistence |
| --- | --- | --- |
| Login | Credential verification and the advertised character-server endpoint | Accounts and pending login handoffs |
| Character | Admission, roster loading, character creation, and selection | Login handoff redemption and character rows |
| Map, planned | Admission to a map, active player state, and map simulation | Character snapshot and save contract to be determined at map admission |

Both running roles use a bounded blocking TCP listener with read and write timeouts. The role handlers exchange typed values with the client protocol module. They never address packet offsets themselves. The database crate owns SQL, connections and persistence constraints. Registration is currently a local CLI command.

Login issues an expiring handoff with two opaque login IDs. The character process admits a connection only when it deletes the matching, unexpired row. That single SQL operation makes concurrent redemption atomic across processes. A new login replaces any pending handoff for the same account. A unique account-and-slot key and a unique character-name key enforce creation constraints across connections. The processes share the database for this first deployment. A separate internal network protocol becomes useful when deployment requires independent data ownership or remote map coordination.

## Wire revisions and findings

`core/src/protocol/v20260219.rs` owns the supported client framing and layouts. Each listener starts with one known revision. A future revision should get its own layout and byte fixtures, and reuse account and character rules. First determine how that client identifies its revision, including whether separate listener configuration is needed.

- TCP reads use exact packet lengths. In particular, the optional login digest packet and the credential packet may arrive together or separately. Replies use complete writes.
- Character admission sends the raw account ID and then the slot, roster, page-notice, block-list, and PIN packets. The page reply uses the same 175-byte record format as the initial roster and character creation acknowledgement.
- Rodecomp finalizes paging when a reply contains a count other than three. rAthena sends a header-only follow-up when the count is exactly three. The checked-in one-record capture remains a golden test. [rAthena character exchange](https://github.com/rathena/rathena/blob/master/src/char/char_clif.cpp)
- The configured address in the login reply is the client-visible destination. The IP is written as IPv4 octets and the port is little-endian. The listener bind is configured independently. [rAthena login exchange](https://github.com/rathena/rathena/blob/master/src/login/loginclif.cpp)

## Map server in increments

rAthena's character process validates the account's requested slot and resolves the character's saved map against available map processes. It sends a destination packet, then the client opens a new socket. The map process decodes the connection request and asks the character process to authenticate it before continuing entry. Map loading and the client's load-complete acknowledgement are later steps in the exchange. [Character routing](https://github.com/rathena/rathena/blob/master/src/char/char_clif.cpp) · [Map connection and load acknowledgement](https://github.com/rathena/rathena/blob/master/src/map/clif.cpp) · [Map-to-character authentication](https://github.com/rathena/rathena/blob/master/src/map/chrif.cpp)

1. **Verify the current milestone in Rodecomp.** Start from an empty account, create a character, reconnect, and confirm its slot and appearance. Selecting it should produce the client's map-unavailable message.
2. **One map destination and admission.** Start a separate map process for an explicitly configured destination. From an owned slot, issue an expiring transfer bound to the account, character and destination, then send the character-to-map endpoint response (`0x0ac5`). Rodecomp's next request uses `0x0436` and carries the account ID, character ID and the login auth code. Verify its bytes and whether the initial `0x0283` acknowledgement and `0x02eb` acceptance match the client before building world state. Keep map transfer state separate from the consumed login handoff. [Server destination packet](https://github.com/rathena/rathena/blob/master/src/common/packets.hpp) · [Rodecomp map transition](https://github.com/rodecomp/rodecomp/blob/main/Source/Client/Mode/LoginMode.cpp)
3. **Enter one map.** Supply a spawn position and the minimum initialization packets, then wait for `0x007d` before exposing the player to map events. Verify the client loads and displays the controlled character before implementing movement, visibility, disconnect cleanup, and saves.

The character role currently parses slot selection and sends a map-unavailable result for an owned slot. There is no map listener yet.

## Verification

- Protocol tests cover the captured character page, empty and three-record page termination, and character-create framing with a following packet.
- A disposable MariaDB 11 instance verified both migrations, registration, wrong-password refusal, login and character admission on separate processes, replay refusal, empty roster, character creation, duplicate-slot refusal, map-unavailable selection, and character reload after restarting the character process.
- Rodecomp source was used to check the packet layouts. The current interaction has not been exercised in the live Rodecomp client.
