#!/usr/bin/env python3
"""Lead-only local probe: python3 -I -B run_rust_probe.py /absolute/path/to/rustc.
Builds only into a temporary directory. No network, user config or real credentials.
"""
import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
SOURCE = Path(__file__).with_name('process_group_probe.rs')
ENV = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'HOME': '/nonexistent'}

def alive(pid):
    try:
        return Path('/proc/%d/stat' % pid).read_text().split(') ', 1)[1][0] != 'Z'
    except (OSError, IndexError):
        return False

def command(argv, cwd, timeout=15):
    entry = {'argv': list(map(str, argv)), 'cwd': str(cwd)}
    try:
        p = subprocess.run(argv, cwd=cwd, env=ENV, capture_output=True, text=True, timeout=timeout)
        entry.update(exit=p.returncode, stdout=p.stdout[:2048], stderr=p.stderr[:2048])
    except (OSError, subprocess.TimeoutExpired) as e:
        entry.update(exit=None, error=type(e).__name__)
    return entry

def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_absolute():
        print('usage: run_rust_probe.py /absolute/path/to/rustc', file=sys.stderr)
        return 2
    rustc = sys.argv[1]
    result = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'cwd': str(Path.cwd()),
              'compiler': rustc, 'limitation': 'Direct rustc compilation, no Cargo; this does not establish behavior on the P03 runner baseline Rust 1.97.1; cooperative processes only.',
              'commands': []}
    result['commands'].append(command(['/usr/bin/git', 'rev-parse', 'HEAD'], ROOT))
    result['revision'] = result['commands'][-1].get('stdout', '').strip()
    result['commands'].append(command([rustc, '--version'], ROOT))
    with tempfile.TemporaryDirectory(prefix='p04-rust-group-') as name:
        tmp = Path(name)
        binary = tmp / 'probe'
        helper = tmp / 'helper.py'
        helper.write_text('import subprocess,sys,time\n'
                          'p=subprocess.Popen([sys.executable,"-I","-c","import time; time.sleep(60)"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)\n'
                          'open(sys.argv[1],"w").write(str(p.pid))\n'
                          'time.sleep(60)\n')
        build = command([rustc, '--edition=2024', str(SOURCE), '-o', str(binary)], ROOT, timeout=60)
        result['commands'].append(build)
        cases = []
        if build.get('exit') == 0:
            for mode in ('default', 'foreground'):
                marker = tmp / (mode + '.pid')
                argv = [str(binary), mode, str(helper), str(marker)]
                case = {'argv': argv, 'cwd': str(tmp)}
                proc = None
                pid = None
                try:
                    # The wrapper's separate group permits bounded cleanup even if the launcher hangs.
                    proc = subprocess.Popen(argv, cwd=tmp, env=ENV, stdout=subprocess.PIPE,
                                            stderr=subprocess.PIPE, text=True, process_group=0)
                    out, err = proc.communicate(timeout=7)
                    case.update(exit=proc.returncode, stdout=out[:2048], stderr=err[:2048])
                    pid = int(marker.read_text())
                    time.sleep(0.2)
                    case['grandchild_alive_after_timeout'] = alive(pid)
                    case['ok'] = proc.returncode == 124 and alive(pid) == (mode == 'foreground')
                except (OSError, ValueError, subprocess.TimeoutExpired) as e:
                    case.update(error=type(e).__name__, ok=False)
                finally:
                    # Rust timeout starts its own group, so kill the known synthetic grandchild too.
                    if pid is None:
                        try:
                            pid = int(marker.read_text())
                        except (OSError, ValueError):
                            pass
                    if pid is not None:
                        try:
                            os.killpg(os.getpgid(pid), signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    if proc is not None:
                        try:
                            os.killpg(proc.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        try:
                            proc.communicate(timeout=2)
                        except subprocess.TimeoutExpired:
                            case['cleanup_error'] = 'timeout'
                            case['ok'] = False
                    if pid is not None:
                        for _ in range(20):
                            if not alive(pid):
                                break
                            time.sleep(0.05)
                        case['grandchild_alive_after_cleanup'] = alive(pid)
                        case['ok'] = case.get('ok', False) and not alive(pid)
                cases.append(case)
        result['cases'] = cases
    result['ok'] = build.get('exit') == 0 and len(cases) == 2 and all(c.get('ok') for c in cases)
    print(json.dumps(result, indent=2))
    return 0 if result['ok'] else 1

if __name__ == '__main__':
    sys.exit(main())
