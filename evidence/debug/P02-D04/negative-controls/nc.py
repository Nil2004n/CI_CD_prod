import sys, os, shutil, hashlib, subprocess, re, pathlib
root=pathlib.Path("/home/kali/Desktop/CI_CD_prod"); out=pathlib.Path(sys.argv[1])
S=root/"workbench/crates/domain/src/store"
cases=[
 ("nc1-order-W2-Engine", S/"evidence.rs", None),
 ("nc2-order-W1-Schedule", S/"workflow.rs", ("    pub timezone: IanaTimezone,\n","    pub next_due_at: Option<UtcTimestamp>,\n")),
 ("nc3-eventtype-extra", S/"log.rs", None),
]
def h(p): return hashlib.sha256(p.read_bytes()).hexdigest()
for name,p,swap in cases:
    orig=p.read_text(); before=h(p); bak=out/(name+".orig"); shutil.copy2(p,bak)
    s=orig
    if name.startswith("nc1"):
        m=re.search(r"pub struct Engine \{\n(.*?)\n\}",s,re.S); body=m.group(1)
        # swap first two field declarations (with their doc lines) 
        fields=re.findall(r"((?:    ///[^\n]*\n|    #\[[^\n]*\n)*    pub [a-z_]+: [^\n]+,\n?)",body+"\n")
        nb=body+"\n"; nb=nb.replace(fields[0]+fields[1],fields[1]+fields[0],1); s=s.replace(body+"\n",nb,1) if (body+"\n") in s else s.replace(body,nb.rstrip("\n"),1)
    elif name.startswith("nc2"):
        a,b=swap; assert a in s and b in s
        s=s.replace(a,"@@A@@",1).replace(b,a,1).replace("@@A@@",b,1)
    else:
        m=re.search(r"(EventType,\s*\d+,\s*\{)",s); assert m, "closed_enum EventType not found"
        cnt=re.search(r"EventType,\s*(\d+),",s); n=int(cnt.group(1))
        s=s.replace(f"EventType, {n},",f"EventType, {n+1},",1).replace(m.group(1).replace(str(n),str(n+1)), m.group(1).replace(str(n),str(n+1))+'\n        /// Negative control.\n        NcFake => "NcFake",',1)
    assert s!=orig, name+": mutation not applied"
    p.write_text(s)
    r=subprocess.run(["python3","-I","-B","dev/p02_check.py","test","--slot","lead","--package","domain","--out",str(out/name)],cwd=root,capture_output=True,text=True)
    shutil.copyfile(bak,p); os.utime(p, None); after=h(p)  # fresh mtime: cargo must rebuild (lead harness fix, P02-D05)
    log=(out/name/"test.log").read_text(errors="replace") if (out/name/"test.log").exists() else ""
    failed=sorted(set(re.findall(r"^test (\S+) \.\.\. FAILED",log,re.M)))
    comp="error[E" in log
    print(f"{name}: runner_exit={r.returncode} compile_error={comp} failed_tests={failed} restored={'OK' if after==before else 'MISMATCH'}")
