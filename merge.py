import json


def operands(op):
    return op
    # match op:
    #     case "label":
    #         return "label"
    #     case "target":
    #         return "target"
    #     case "$t1":
    #         return ""
    #     case "$t2":
    #         return ""
    #     case "$t3":
    #         return ""
    #     case "$f1":
    #         return ""
    #     case "$f2":
    #         return ""
    #     case "100000":
    #         return ""
    #     case "-100":
    #         return ""
    #     case "100":
    #         return ""
    #     case "10":
    #         return ""
    #     case "100000($t2)":
    #         return ""
    #     case "100($t2)":
    #         return ""
    #     case "($t2)":
    #         return ""

    # s/label(\$t2)//g
    # s/label+100000(\$t2)//g
    # s/label+100000//g


inst_file = "./resources/instructions.json"
pseudo_file = "./resources/pseudo-instructions.json"
merge_file = "./resources/merged.json"

with open(inst_file) as f:
    inst: dict = json.load(f)
with open(pseudo_file) as f:
    pseudo: dict = json.load(f)

merged = {}

for key, value in pseudo.items():
    merged[key] = {
        "format": "",
        "native": [],
        "pseudo": [],
    }

for key, value in inst.items():
    merged[key] = {
        "format": value["format"],
        "native": [],
        "pseudo": [],
    }
    for p in value["variants"]:
        merged[key]["native"].append(
            {
                "description": p["description"],
                "operands": list(map(operands, p["operands"])),
                "code": p["code"],
            }
        )

for key, value in pseudo.items():
    for p in value:
        pattern = p["pattern"].split(" ", 1)
        if len(pattern) > 1:
            pattern = pattern[1]
        else:
            pattern = ""
        pattern = pattern.replace(" ", "").split(",")

        merged[key]["pseudo"].append(
            {
                "description": p["description"],
                "operands": list(map(operands, pattern)),
                "replacement": p["replacement"],
            }
        )

with open(merge_file, "w") as f:
    json.dump(merged, f)
