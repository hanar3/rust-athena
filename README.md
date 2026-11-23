# rust-athena
Ragnarok Online login and char servers reimplemented in rust

[WIP] For learning purpouses only -- Creates a local server that implements most of the packets used by login and character selection for Ragnarok Online.
The server also creates a default character.

## Tooling
[Wireshark](https://www.wireshark.org/) - Monitor network protocol

[WARP](https://github.com/Neo-Mind/WARP) - Disable packet encryption on client-side

Inspired by [rAthena](https://github.com/rathena/rathena)

## Local 2026-02-19 demo

Run `cargo run --locked -- --demo-20260219` and point a compatible client at
`127.0.0.1:6900`. Sign in with the local test credentials `tester2` / `tester2`,
then select the **Rust Athena** character server. Slot zero contains **Ferris**.
The demo listens only on loopback and uses in-memory data. Building still
requires the existing MySQL client library dependency.

Packet reference: rAthena revision `226bd83d39584bc91b6bf133f40bce95aa1f178c`,
built with `PACKETVER=20260219`. The checked-in fixture records the one-character
`0x0b72` response for the seeded account.
