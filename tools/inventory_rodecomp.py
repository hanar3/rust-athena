#!/usr/bin/env python3
"""Export Rodecomp packet names and registrations without inferring implementation."""

import argparse
import csv
import re
import subprocess
import sys
from collections import defaultdict

PREFIXES = {
    "CA": "login:client->server",
    "AC": "login:server->client",
    "CH": "character:client->server",
    "HC": "character:server->client",
    "CZ": "map:client->server",
    "ZC": "map:server->client",
}


def source(repo, revision, name):
    path = f"Source/Network/Packets/{name}"
    return subprocess.check_output(
        ["git", "-C", repo, "show", f"{revision}:{path}"], text=True
    )


def inventory(repo, revision):
    names = defaultdict(list)
    registrations = defaultdict(list)
    for line in source(repo, revision, "PacketId.h").splitlines():
        if not line.strip().startswith("ID_"):
            continue
        match = re.fullmatch(r"\s*(ID_\w+)\s*=\s*(0x[0-9a-fA-F]+|\d+),?\s*(?://.*)?", line)
        if match is None:
            raise ValueError(f"Unrecognized packet ID declaration: {line}")
        name, opcode = match.groups()
        names[int(opcode, 0)].append(name)

    for line in source(repo, revision, "PacketRegistrations.inc").splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"\s*\{(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(\d+)\},?\s*", line)
        if match is None:
            raise ValueError(f"Unrecognized registration: {line}")
        record, variable, length, opcode = map(int, match.groups())
        registrations[opcode].append((record, variable, length))

    for opcode in sorted(names.keys() | registrations.keys()):
        aliases = names[opcode]
        frames = registrations[opcode]
        hints = {PREFIXES.get(name.split("_")[1], "unknown") for name in aliases}
        if frames:
            _, variable, length = frames[0]
            framing = f"fixed:{length}" if length >= 0 else f"variable:{variable}" if length == -1 else "unknown"
        else:
            framing = "unregistered"
        yield [
            f"0x{opcode:04x}",
            "|".join(aliases),
            "|".join(sorted(hints)) or "unknown",
            framing,
            "|".join(":".join(map(str, frame)) for frame in frames),
        ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rodecomp", help="path to a Rodecomp Git checkout")
    parser.add_argument("--revision", default="ce6d40783aa3a98043a232c28678ae32f9368dd5")
    args = parser.parse_args()
    writer = csv.writer(sys.stdout, lineterminator="\n")
    writer.writerow(["opcode", "aliases", "prefix_hints", "first_registration_framing", "registrations_record_variable_length"])
    writer.writerows(inventory(args.rodecomp, args.revision))


if __name__ == "__main__":
    main()
