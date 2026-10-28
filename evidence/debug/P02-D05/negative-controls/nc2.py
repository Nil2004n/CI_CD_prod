import sys, os, shutil, hashlib, subprocess, re, pathlib
root=pathlib.Path("/home/kali/Desktop/CI_CD_prod"); out=pathlib.Path(sys.argv[1]); C=root/"workbench/crates"
cases=[ # name, file, old, new, package, expected failing test
 ("nc4-derived-store-enum", C/"domain/src/store/tools.rs", None, "\n/// Negative control.\n#[derive(Debug, serde::Deserialize)]\npub enum NcProbe {\n    /// A.\n    A,\n}\n", "domain", "no_store_enum_derives_deserialize"),
 ("nc5-envelope-name", C/"domain/src/contract.rs", "        fired_at: UtcTimestamp,", "        #[serde(rename = \"at\")]\n        fired_at: UtcTimestamp,", "domain", "no_event_payload_uses_an_envelope_field_name"),
 ("nc6-version-class", C/"persistence/src/read.rs", '["1.0", "1.1", "1.2", "1.3"]', '["1.0", "1.1", "1.2", "9.9"]', "persistence", "version_classes_follow_request_6_1"),
 ("nc7-problem-cap", C/"persistence/src/validate.rs", "if self.items.len() < MAX_PROBLEMS {", "if self.items.len() <= MAX_PROBLEMS {", "persistence", "problems_are_capped_and_truncation_carries_over"),
]
def h(p): return hashlib.sha256(p.read_bytes()).hexdigest()
for name,p,old,new,pkg,want in cases:
    orig=p.read_text(); before=h(p); bak=out/(name+".orig"); shutil.copy2(p,bak)
    if old is None: s=orig+new
    else:
        assert orig.count(old)==1, name; s=orig.replace(old,new,1)
    p.write_text(s)
    r=subprocess.run(["python3","-I","-B","dev/p02_check.py","test","--slot","lead","--package",pkg,"--out",str(out/name)],cwd=root,capture_output=True,text=True)
    shutil.copyfile(bak,p); os.utime(p, None); after=h(p)  # fresh mtime: cargo must rebuild (lead harness fix, P02-D05)
    log=(out/name/"test.log").read_text(errors="replace") if (out/name/"test.log").exists() else ""
    failed=sorted(set(re.findall(r"^test (\S+) \.\.\. FAILED",log,re.M)))
    print(f"{name}: exit={r.returncode} compile_error={'error[E' in log} failed={failed} expected_hit={any(want in f for f in failed)} restored={'OK' if after==before else 'MISMATCH'}")
