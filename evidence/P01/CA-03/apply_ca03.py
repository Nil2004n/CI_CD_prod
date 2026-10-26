"""CA-03 (owner approval 2026-10-26): canonical tool-availability vocabulary; store contract 1.3 -> 1.4.
ToolManifest.availability = ready, missing, incompatible, unconfigured, unavailable, deferred ('unavailable' added).
'denied' is a permission outcome (approval/coverage records), not a tool state. Wrapper protocol unchanged (1.1)."""
import json, sys
root = sys.argv[1]
def dump(o): return json.dumps(o, indent=2, ensure_ascii=False) + "\n"
p = f"{root}/schemas/workbench-store-v1.4.schema.json"
s = json.load(open(p)); d = s["$defs"]
n_store = 0
for name, df in d.items():
    pr = df.get("properties", {})
    if pr.get("schema_version") == {"const": "1.3"}: pr["schema_version"] = {"const": "1.4"}; n_store += 1
av = d["ToolManifest"]["properties"]["availability"]
assert av["enum"] == ["ready", "missing", "incompatible", "deferred", "unconfigured"], av
av["enum"] = ["ready", "missing", "incompatible", "unconfigured", "unavailable", "deferred"]
s["title"] = "PWT Workbench Store Contract v1.4"
s["$comment"] = ("Store contract 1.4 / wrapper protocol 1.1 = store 1.2 / protocol 1.0 plus pre-implementation amendment CA-01 "
                 "(2026-10-23, identifiers retained) plus amendment CA-02 (2026-10-24, store 1.3 / protocol 1.1) plus amendment "
                 "CA-03 (2026-10-26, canonical tool availability; store 1.4, protocol unchanged). See 05_Database_Data_Schema.md "
                 "sections 19 and 22-24.")
open(p, "w").write(dump(s))
for name, ref in [("wrapper-request", "WrapperRequest"), ("wrapper-control", "WrapperControl")]:
    open(f"{root}/schemas/{name}-v1.1.schema.json", "w").write(dump({"$schema": s["$schema"], "$comment": s["$comment"],
        "title": f"PWT {ref} v1.1", "$ref": f"#/$defs/{ref}", "$defs": d}))
mp = f"{root}/05_Database_Data_Schema.md"; t = open(mp).read(); lines = t.split("\n")
st = lines.index("```json", 100); en = lines.index("```", st + 1)
lines[st + 1:en] = dump(s).rstrip("\n").split("\n")
t = "\n".join(lines)
head, tail = t.split("## 8. Example `repo.json`", 1)
ex8_10, rest = tail.split("## 11. Runtime semantic validation", 1)
c = ex8_10.count('"schema_version": "1.3",'); ex8_10 = ex8_10.replace('"schema_version": "1.3",', '"schema_version": "1.4",')
open(mp, "w").write(head + "## 8. Example `repo.json`" + ex8_10 + "## 11. Runtime semantic validation" + rest)
print(f"store consts updated={n_store} availability enum={av['enum']} store examples={c}")
