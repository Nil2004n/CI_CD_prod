"""Version the CA-02 contract: store 1.2 -> 1.3, wrapper protocol 1.0 -> 1.1 (owner Q-03 decision, 2026-10-24)."""
import json, sys
root = sys.argv[1]
def dump(o): return json.dumps(o, indent=2, ensure_ascii=False) + "\n"
p = f"{root}/schemas/workbench-store-v1.3.schema.json"
s = json.load(open(p)); d = s["$defs"]
n_store = n_proto = 0
for name, df in d.items():
    pr = df.get("properties", {})
    if pr.get("schema_version") == {"const": "1.2"}: pr["schema_version"] = {"const": "1.3"}; n_store += 1
    if pr.get("protocol_version") == {"const": "1.0"}: pr["protocol_version"] = {"const": "1.1"}; n_proto += 1
s["title"] = "PWT Workbench Store Contract v1.3"
s["$comment"] = ("Store contract 1.3 / wrapper protocol 1.1 = store 1.2 / protocol 1.0 plus pre-implementation amendment CA-01 "
                 "(2026-10-23, identifiers retained) plus amendment CA-02 (2026-10-24, versioned by owner decision Q-03). "
                 "See 05_Database_Data_Schema.md sections 22-23 and 19 (records under 1.2/1.0 are not valid 1.3/1.1 records).")
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
c = ex8_10.count('"schema_version": "1.2",'); ex8_10 = ex8_10.replace('"schema_version": "1.2",', '"schema_version": "1.3",')
pre23, post23 = rest.split("## 23. Contract amendment CA-02", 1)
c2 = post23.count('"protocol_version": "1.0",'); post23 = post23.replace('"protocol_version": "1.0",', '"protocol_version": "1.1",')
open(mp, "w").write(head + "## 8. Example `repo.json`" + ex8_10 + "## 11. Runtime semantic validation" + pre23 + "## 23. Contract amendment CA-02" + post23)
print(f"store consts updated={n_store} protocol consts updated={n_proto} store examples={c} §23 example={c2}")
