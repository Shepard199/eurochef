#!/usr/bin/env python3
import argparse
import csv
import json
from pathlib import Path

EXTERNAL_CURRENT_UIDS = {0x47000003, 0x47000023}


def parse_hex(value: str) -> int:
    return int(value, 16)


def load_missions(path: Path):
    with path.open(newline="", encoding="utf-8") as f:
        return [
            {
                "mission_uid": parse_hex(row["mission_uid"]),
                "objective_uid": parse_hex(row["objective_uid"]),
                "hud_item_uid": parse_hex(row["hud_item_uid"]),
                "mission_text_uid": parse_hex(row["mission_text_uid"]),
            }
            for row in csv.DictReader(f)
        ]


def load_inventory(path: Path):
    with path.open(newline="", encoding="utf-8") as f:
        rows = []
        for row in csv.DictReader(f):
            uid = parse_hex(row["uid"])
            word_14 = parse_hex(row["word_14"])
            rows.append(
                {
                    "uid": uid,
                    "word_14": word_14,
                    "target": int(row["target"]),
                    "storage": "bitset" if word_14 & 1 else "counter",
                    "uses_external_current": uid in EXTERNAL_CURRENT_UIDS,
                }
            )
        return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("missions_csv", type=Path)
    parser.add_argument("inventory_csv", type=Path)
    parser.add_argument("output_dir", type=Path)
    args = parser.parse_args()

    missions = load_missions(args.missions_csv)
    inventory = load_inventory(args.inventory_csv)
    inventory_by_uid = {row["uid"]: row for row in inventory}

    joined = []
    unresolved = []
    for mission in missions:
        objective_uid = mission["objective_uid"]
        if objective_uid == 0xFFFFFFFF:
            continue
        definition = inventory_by_uid.get(objective_uid)
        if definition is None:
            unresolved.append(objective_uid)
            continue
        joined.append(
            {
                **mission,
                "target": definition["target"],
                "storage": definition["storage"],
                "uses_external_current": definition["uses_external_current"],
            }
        )

    summary = {
        "schema": "robots-mission-objective-census-v1",
        "mission_definition_count": len(missions),
        "mapped_mission_count": sum(m["objective_uid"] != 0xFFFFFFFF for m in missions),
        "inventory_definition_count": len(inventory),
        "joined_objective_count": len(joined),
        "unresolved_objective_uids": [f"0x{uid:08X}" for uid in sorted(set(unresolved))],
        "external_current_mission_objectives": [
            f"0x{row['objective_uid']:08X}" for row in joined if row["uses_external_current"]
        ],
        "counter_objective_count": sum(row["storage"] == "counter" for row in joined),
        "bitset_objective_count": sum(row["storage"] == "bitset" for row in joined),
    }

    args.output_dir.mkdir(parents=True, exist_ok=True)
    (args.output_dir / "summary.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    with (args.output_dir / "mission_objectives.tsv").open("w", encoding="utf-8", newline="") as f:
        writer = csv.writer(f, delimiter="\t", lineterminator="\n")
        writer.writerow(
            [
                "mission_uid",
                "objective_uid",
                "hud_item_uid",
                "mission_text_uid",
                "target",
                "storage",
                "uses_external_current",
            ]
        )
        for row in joined:
            writer.writerow(
                [
                    f"0x{row['mission_uid']:08X}",
                    f"0x{row['objective_uid']:08X}",
                    f"0x{row['hud_item_uid']:08X}",
                    f"0x{row['mission_text_uid']:08X}",
                    row["target"],
                    row["storage"],
                    str(row["uses_external_current"]).lower(),
                ]
            )

    print(json.dumps(summary, sort_keys=True))
    return 1 if unresolved else 0


if __name__ == "__main__":
    raise SystemExit(main())
