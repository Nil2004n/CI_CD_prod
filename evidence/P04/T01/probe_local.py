#!/usr/bin/env python3
"""Local-only P04 T01 observations. Run: python3 -I -B evidence/P04/T01/probe_local.py > /tmp/p04-t01-probe.json
Synthetic fixtures only. No sockets, user Git configuration, or inherited environment is read by children.
"""
import json
from datetime import datetime, timezone
import os
import pathlib
import pwd
import signal
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
ENV = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}
records = []

def run(argv, cwd, env=ENV, timeout=10, full_stdout=False):
    entry = {'argv': [str(x) for x in argv], 'cwd': str(cwd)}
    try:
        p = subprocess.run(argv, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE, timeout=timeout, check=False)
        entry.update(exit=p.returncode, stdout=p.stdout if full_stdout else p.stdout[:4096], stderr=p.stderr[:1024])
    except (OSError, subprocess.TimeoutExpired) as e:
        entry.update(exit=None, error=type(e).__name__)
    records.append(entry)
    return entry

def alive(pid):
    try:
        # A zombie has terminated, even if its parent has not reaped it yet.
        return pathlib.Path('/proc/%d/stat' % pid).read_text().split(') ', 1)[1][0] != 'Z'
    except (OSError, IndexError):
        return False

def group_case(tmp, foreground):
    marker = tmp / ('foreground.pid' if foreground else 'default.pid')
    helper = tmp / 'helper.py'
    # Both processes cooperate with TERM; no daemonisation or setsid.
    helper.write_text('import os,subprocess,sys,time\n'
                      'p=subprocess.Popen([sys.executable,"-I","-c","import time; time.sleep(60)"])\n'
                      'open(sys.argv[1],"w").write(str(p.pid))\n'
                      'time.sleep(60)\n')
    cmd = ['/usr/bin/timeout', '-k', '1s'] + (['--foreground'] if foreground else []) + [
        '2s', sys.executable, '-I', '-B', str(helper), str(marker)]
    entry = {'argv': cmd, 'cwd': str(tmp), 'process_group': 0}
    proc = None
    pid = None
    error_file = tmp / ('foreground.stderr' if foreground else 'default.stderr')
    try:
        with error_file.open('wb') as errors:
            proc = subprocess.Popen(cmd, cwd=tmp, env=ENV, stdout=subprocess.DEVNULL,
                                    stderr=errors, process_group=0)
        entry['exit'] = proc.wait(timeout=6)
        pid = int(marker.read_text())
        time.sleep(0.2)
        entry['grandchild_alive_after_timeout'] = alive(pid)
        entry['expected'] = foreground
        entry['ok'] = entry['exit'] == 124 and entry['grandchild_alive_after_timeout'] == foreground
    except (OSError, ValueError, subprocess.TimeoutExpired) as e:
        entry.update(exit=None, error=type(e).__name__, ok=False)
    finally:
        if proc is not None:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                entry['cleanup_error'] = 'timeout'
        if pid is not None:
            for _ in range(20):
                if not alive(pid):
                    break
                time.sleep(0.05)
            entry['grandchild_alive_after_cleanup'] = alive(pid)
            entry['ok'] = entry.get('ok', False) and not entry['grandchild_alive_after_cleanup']
        if error_file.exists():
            entry['stderr'] = error_file.read_text(errors='replace')[:1024]
    records.append(entry)
    return entry

def main():
    result = {'schema': 1, 'utc': datetime.now(timezone.utc).isoformat(), 'cwd': str(pathlib.Path.cwd()), 'repository': str(ROOT),
              'python_version': sys.version.split()[0], 'commands': records,
              'limitations': ['Local cooperative processes only; setsid escape and real network are not tested.',
                              'Cache presence is not proof of offline dependency completeness.']}
    rev = run(['/usr/bin/git', 'rev-parse', 'HEAD'], ROOT)
    result['revision'] = rev.get('stdout', '').strip()
    # Only count status lines; never emit paths or config values.
    try:
        p = subprocess.run(['/usr/bin/git', 'status', '--porcelain=v1', '-z'], cwd=ROOT,
                           env={**ENV, 'HOME': '/nonexistent', 'GIT_CONFIG_NOSYSTEM': '1',
                                'GIT_CONFIG_GLOBAL': '/dev/null'}, stdout=subprocess.PIPE,
                           stderr=subprocess.DEVNULL, timeout=10, check=False)
        result['dirty_state'] = {'exit': p.returncode, 'entries': len([x for x in p.stdout.split(b'\0') if x])}
    except (OSError, subprocess.TimeoutExpired) as e:
        result['dirty_state'] = {'exit': None, 'error': type(e).__name__}
    for cmd in (['/usr/bin/ssh', '-V'], ['/usr/bin/timeout', '--version'], ['/usr/bin/git', '--version']):
        run(cmd, ROOT)
    tc = pathlib.Path('/home/kali/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin')
    cache = pathlib.Path('/home/kali/.local/share/pwt-dev-tools/_cargo-home')
    result['portability'] = {'runner_toolchain_dir_exists': tc.is_dir(), 'runner_cargo_home_exists': cache.is_dir(),
                             'runner_registry_cache_exists': (cache / 'registry' / 'cache').is_dir(),
                             'runner_hardcoded_paths': [str(tc), str(cache)]}
    if (tc / 'rustc').is_file():
        v = run([str(tc / 'rustc'), '--version'], ROOT)
        result['portability']['rust_1_97_1'] = v.get('exit') == 0 and v.get('stdout', '').split()[1:2] == ['1.97.1']
    else:
        result['portability']['rust_1_97_1'] = None
    with tempfile.TemporaryDirectory(prefix='p04-t01-') as name:
        tmp = pathlib.Path(name)
        home = tmp / 'synthetic-home'
        home.mkdir()
        config = tmp / 'ssh_config'
        config.write_text('Host *\n    IdentityFile ~/.ssh/p04-synthetic-key\n    IdentityAgent ~/.ssh/p04-synthetic-agent\n')
        config.chmod(0o600)
        ssh = run(['/usr/bin/ssh', '-G', '-F', str(config), 'p04.invalid'], tmp,
                  {**ENV, 'HOME': str(home)}, full_stdout=True)
        identities = [line.split(' ', 1)[1] for line in ssh.get('stdout', '').splitlines()
                      if line.lower().startswith('identityfile ')]
        agents = [line.split(' ', 1)[1] for line in ssh.get('stdout', '').splitlines()
                  if line.lower().startswith('identityagent ')]
        passwd_home = pwd.getpwuid(os.getuid()).pw_dir
        expected_passwd = str(pathlib.Path(passwd_home) / '.ssh/p04-synthetic-agent')
        expected_env = str(home / '.ssh/p04-synthetic-agent')
        # ssh -G may leave both values literal. Such output proves no expansion.
        if ssh.get('exit') != 0:
            observation = 'fail'
        elif expected_passwd != expected_env and expected_passwd in agents:
            observation = 'pass'
        else:
            observation = 'inconclusive'
        ssh['stdout'] = ''
        result['ssh_home'] = {'synthetic_home': str(home), 'passwd_home': passwd_home,
                              'identityfiles': identities, 'identity_agents': agents,
                              'ssh_exit': ssh.get('exit'), 'status': observation,
                              'meaning': 'pass only when ssh -G expands IdentityAgent to the distinct passwd home; literal or other output is inconclusive'}
        result['groups'] = {'default': group_case(tmp, False), 'foreground_control': group_case(tmp, True)}
    outcomes = [result['ssh_home']['status']] + [
        'pass' if case.get('ok') else 'fail' for case in result['groups'].values()]
    result['counts'] = {key: outcomes.count(key) for key in ('pass', 'fail', 'inconclusive')}
    print(json.dumps(result, indent=2))
    return 1 if result['counts']['fail'] else (2 if result['counts']['inconclusive'] else 0)

if __name__ == '__main__':
    sys.exit(main())
