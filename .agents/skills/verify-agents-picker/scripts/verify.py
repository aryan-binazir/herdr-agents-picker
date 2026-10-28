#!/usr/bin/env python3
"""Drive the release TUI on an isolated tmux server; retain proof artifacts."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
BINARY = ROOT / 'target/release/agents-picker'


def doctor(binary):
    for name in ('tmux',):
        if not shutil.which(name):
            raise RuntimeError(f'{name} is missing')
    result = subprocess.run([str(binary), '--help'], capture_output=True, text=True, timeout=5)
    if result.returncode or 'run the picker TUI' not in result.stdout:
        raise RuntimeError(f'binary doctor failed: {result.stderr}')
    return {'binary': str(binary), 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
            'help': result.stdout, 'tmux': subprocess.check_output(['tmux', '-V'], text=True).strip()}


def fixture(args):
    directory = Path(os.environ['PICKER_VERIFY_SCRATCH'])
    with (directory / 'calls.jsonl').open('a') as out:
        out.write(json.dumps({'time': time.time(), 'args': args}) + '\n')
    state = json.loads((directory / 'fixture.json').read_text())
    if args == ['agent', 'list']:
        result = {'agents': state['agents']}
    elif args == ['workspace', 'list']:
        result = {'workspaces': [{'workspace_id': 'verify-w', 'label': state.get('workspace_label', 'Verify workspace'), 'tab_count': 2}]}
    elif args == ['tab', 'list']:
        result = {'tabs': [{'tab_id': 'verify-t', 'label': state.get('tab_label', 'Verification checks'), 'number': 1}]}
    elif len(args) == 3 and args[0] in ('workspace', 'tab', 'agent') and args[1] == 'focus':
        result = {}
    elif args == ['plugin', 'pane', 'open', '--plugin', 'yxhta.agents-picker', '--entrypoint', 'picker', '--focus']:
        result = {}
    else:
        print('unsupported fixture command: ' + repr(args), file=sys.stderr)
        return 2
    print(json.dumps({'result': result}))
    return 0


def run_case(feature, binary, evidence, case=None):
    evidence.mkdir(parents=True, exist_ok=False)
    scratch = Path(tempfile.mkdtemp(prefix='picker-verify-', dir='/private/tmp' if sys.platform == 'darwin' else '/tmp'))
    socket = scratch / 'tmux.sock'
    script = Path(__file__).resolve()
    wrapper = scratch / 'herdr-fixture'
    wrapper.write_text('#!/bin/sh\nexec ' + shlex.join([sys.executable, str(script), 'fixture']) + ' "$@"\n')
    wrapper.chmod(0o755)
    agents = [{'agent': 'codex', 'agent_status': 'idle', 'name': name,
               'pane_id': f'verify-{name.lower()}', 'terminal_id': f'term-{name.lower()}',
               'workspace_id': 'verify-w', 'tab_id': 'verify-t', 'cwd': '/verify/' + name.lower()}
              for name in ('Alpha', 'Beta')]
    if feature == 'numbers':
        agents = [{'agent': 'codex', 'agent_status': 'idle',
                   'name': f'Thread{i:02}', 'terminal_title_stripped': f'Title{i:02}',
                   'pane_id': f'verify-row{i:02}', 'terminal_id': f'term-{i}',
                   'workspace_id': 'verify-w', 'tab_id': 'verify-t', 'cwd': '/verify'}
                  for i in range(1, 15)]
        agents[11]['terminal_title_stripped'] = None
        agents[12]['name'] = None
        agents[12]['terminal_title_stripped'] = 'Thread13'
        if case == 'short':
            agents = agents[:2]
        elif case == 'empty':
            agents = []
        elif case in ('reorder', 'selection', 'glyphs'):
            for agent, status in zip(agents, ('blocked', 'done', 'working', 'idle', 'unknown')):
                agent['agent_status'] = status
    state = {'agents': agents}
    (scratch / 'fixture.json').write_text(json.dumps(state))
    (scratch / 'config.toml').write_text('[ui]\nagent_panel_sort = "spaces"\n')
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('HERDR_', 'TMUX')) and key not in ('BASH_ENV', 'ENV')}
    env.update(HERDR_BIN_PATH=str(wrapper), HERDR_CONFIG_PATH=str(scratch / 'config.toml'),
               HERDR_PLUGIN_STATE_DIR=str(scratch / 'state'), HERDR_PANE_ID='verify-picker',
               HERDR_PLUGIN_ID='yxhta.agents-picker', PICKER_VERIFY_SCRATCH=str(scratch), TERM='xterm-256color')
    env.pop('NO_COLOR', None)
    if case in ('reorder', 'selection', 'glyphs'):
        (scratch / 'config.toml').write_text('[ui]\nagent_panel_sort = "priority"\n')
    journal = evidence / 'actions.jsonl'
    expected_focus = None
    started = False

    def record(action, **fields):
        with journal.open('a') as out:
            out.write(json.dumps({'time': time.time(), 'feature': feature, 'action': action, **fields}) + '\n')

    def tmux(*args, check=True):
        result = subprocess.run(['tmux', '-f', '/dev/null', '-S', str(socket), *args], env=env,
                                capture_output=True, text=True, timeout=8)
        record('tmux', argv=list(args), exit_code=result.returncode, stderr=result.stderr)
        if check and result.returncode:
            raise RuntimeError(f'tmux {args!r}: {result.stderr}')
        return result

    def screen():
        return tmux('capture-pane', '-p', '-t', 'verify:0.0').stdout

    def snapshot(name):
        (evidence / (name + '.txt')).write_text(screen())
        (evidence / (name + '.ansi')).write_text(tmux('capture-pane', '-p', '-e', '-t', 'verify:0.0').stdout)
        record('capture', name=name)

    def wait_for(text):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if text in screen():
                return
            time.sleep(.1)
        snapshot('failure')
        raise AssertionError(f'screen did not contain {text!r}')

    def keys(*names):
        record('keys', keys=names)
        tmux('send-keys', '-t', 'verify:0.0', *names)

    def text(value):
        record('text', value=value)
        tmux('send-keys', '-l', '-t', 'verify:0.0', value)

    def wait_exit():
        deadline = time.monotonic() + 8
        while not (scratch / 'exit.json').exists() and time.monotonic() < deadline:
            time.sleep(.1)
        assert (scratch / 'exit.json').exists(), 'picker did not exit after focus key'
        result = json.loads((scratch / 'exit.json').read_text())
        assert result['exit_code'] == 0, result
        (evidence / 'exit.json').write_text(json.dumps(result))

    def calls():
        path = scratch / 'calls.jsonl'
        return [json.loads(line)['args'] for line in path.read_text().splitlines()] if path.exists() else []

    def no_focus():
        assert not [call for call in calls() if len(call) > 1 and call[1] == 'focus'], calls()
        assert not (scratch / 'exit.json').exists(), 'picker unexpectedly exited'

    def rows():
        return [(match.group(1), int(match.group(2))) for line in screen().splitlines()
                if (match := re.match(r'^\s*│?\s*([0-9]?)\s*[◉●✓○⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]\s+.*?Thread(\d+)', line))]

    def labeled():
        rendered = rows()
        assert rendered, f'no numbered rows rendered: {screen()}'
        assert [label for label, _ in rendered] == list('1234567890'[:len(rendered)]) + [''] * max(0, len(rendered) - 10), rendered
        return rendered

    def digit(label):
        nonlocal expected_focus
        rendered = labeled()
        target = next(row for number, row in rendered if number == label)
        expected_focus = f'verify-row{target:02}'
        snapshot('before-digit')
        record('expected-target', digit=label, pane=expected_focus, visible_rows=rendered)
        text(label)

    def noop_digit(label):
        assert label not in [number for number, _ in rows()], rows()
        text(label)
        refresh_observation_seconds = 2.3
        time.sleep(refresh_observation_seconds)
        no_focus()
        snapshot('unavailable-' + label)

    try:
        (evidence / 'doctor.json').write_text(json.dumps(doctor(binary), indent=2))
        record('launch', binary=str(binary), socket=str(socket), scratch=str(scratch), case=case)
        tmux('new-session', '-d', '-s', 'verify', '-x', '160', '-y', '32', '/bin/sleep 180')
        started = True
        tmux('set-option', '-t', 'verify', 'remain-on-exit', 'on')
        command = shlex.join([sys.executable, str(script), 'child', str(binary)])
        tmux('respawn-pane', '-k', '-t', 'verify:0.0', command)
        wait_for(f'{len(agents)}/{len(agents)}')
        if agents:
            wait_for('Thread' if feature == 'numbers' else 'Alpha')
        snapshot('ready')
        if feature == 'search':
            keys('/')
            text('Beta')
            wait_for('1/2')
            wait_for('Beta')
            assert 'Alpha' not in screen()
            snapshot('match')
            keys('C-u')
            text('zznomatchzz')
            wait_for('No matches for "zznomatchzz"')
            snapshot('no-match')
            keys('Escape')
            wait_for('2/2')
            snapshot('cancel-search')
            keys('q')
        elif feature == 'navigation':
            keys('j')
            snapshot('selected-beta')
            expected_focus = 'verify-beta'
            keys('Enter')
        elif feature == 'refresh':
            keys('j')
            state['agents'] = agents[:1]
            (scratch / 'fixture.json').write_text(json.dumps(state))
            keys('r')
            wait_for('1/1')
            snapshot('manual-reload')
            state['agents'] = agents
            (scratch / 'fixture.json').write_text(json.dumps(state))
            wait_for('2/2')
            snapshot('automatic-reload')
            keys('q')
        elif feature == 'launch':
            assert 'Verify workspace · Verification checks' in screen(), 'workspace/tab label is clipped despite spare width'
            state.update(workspace_label='日本語', tab_label='作業')
            (scratch / 'fixture.json').write_text(json.dumps(state))
            keys('r')
            wait_for('日本語 · 作業')
            snapshot('wide-workspace-label')
            keys('q')
        elif feature == 'numbers':
            if case == 'empty':
                wait_for('No agent panes found')
                for label in '1234567890':
                    noop_digit(label)
                keys('q')
            else:
                rendered = labeled()
                assert 'Agents' not in screen(), 'inner Agents title remains'
                assert all(screen().count(corner) == 1 for corner in '╭╮╰╯'), 'expected one rounded panel'
                if case.startswith('digit-'):
                    assert len(rendered) >= 10, rendered
                    assert [row for _, row in rendered[:10]] == list(range(1, 11)), rendered
                    digit(case[-1])
                elif case == 'short':
                    for label in '34567890':
                        noop_digit(label)
                    digit('2')
                elif case == 'tiny':
                    tmux('resize-window', '-t', 'verify', '-x', '100', '-y', '11')
                    time.sleep(.25)
                    snapshot('small-viewport')
                    assert 0 < len(labeled()) < 9, rows()
                    noop_digit('9')
                    noop_digit('0')
                    digit('1')
                elif case == 'zero-height':
                    tmux('resize-window', '-t', 'verify', '-x', '100', '-y', '1')
                    time.sleep(.25)
                    assert not rows(), rows()
                    noop_digit('1')
                    noop_digit('0')
                    keys('q')
                elif case == 'modifiers':
                    # CSI-u encodes Ctrl+digit, which legacy terminals cannot distinguish.
                    for label in '1234567890':
                        keys('M-' + label)
                        tmux('send-keys', '-H', '-t', 'verify:0.0', '1b', '5b', *[f'{byte:02x}' for byte in f'{ord(label)};5u'.encode()])
                    time.sleep(.3)
                    no_focus()
                    snapshot('modified-digits')
                    digit('2')
                elif case == 'search-digits':
                    keys('/')
                    text('10')
                    wait_for('1/14')
                    wait_for('Thread10')
                    no_focus()
                    snapshot('search-digits')
                    keys('Escape')
                    wait_for('14/14')
                    digit('2')
                elif case in ('scroll', 'resize'):
                    tmux('resize-window', '-t', 'verify', '-x', '100', '-y', '13')
                    time.sleep(.2)
                    for _ in range(11):
                        keys('j')
                    wait_for('Thread12')
                    snapshot('scrolled')
                    assert labeled()[0][1] > 1, rows()
                    if case == 'resize':
                        tmux('resize-window', '-t', 'verify', '-x', '120', '-y', '11')
                        time.sleep(.25)
                        snapshot('resized')
                    digit('2')
                elif case in ('reorder', 'selection'):
                    keys('j')
                    state['agents'][-1]['agent_status'] = 'blocked'
                    (scratch / 'fixture.json').write_text(json.dumps(state))
                    keys('r')
                    deadline = time.monotonic() + 8
                    while time.monotonic() < deadline and (not rows() or rows()[0][1] != 14):
                        time.sleep(.1)
                    assert labeled()[0][1] == 14, rows()
                    snapshot('status-reordered')
                    if case == 'selection':
                        expected_focus = 'verify-row02'
                        keys('Enter')
                    else:
                        digit('1')
                elif case == 'glyphs':
                    for glyph in '◉●✓○':
                        assert glyph in screen(), (glyph, screen())
                    frames = set()
                    for _ in range(12):
                        frames.update(char for char in screen() if char in '⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏')
                        time.sleep(.12)
                    assert len(frames) > 1, frames
                    assert 'Title01' not in screen(), 'automatic title must not be appended to an explicit name'
                    first = next(line for line in screen().splitlines() if 'Thread01' in line)
                    assert first.index('Thread01') < first.index('Verify') < first.index('codex'), first
                    assert '/verify' not in screen(), 'obsolete cwd column remains'
                    assert 'Thread12' in screen() and 'Thread13' in screen(), 'name-only or title-only thread missing'
                    snapshot('glyphs-and-titles')
                    keys('q')
        wait_exit()
        if feature == 'launch':
            result = subprocess.run([str(binary), '--open'], env=env, capture_output=True, text=True, timeout=8)
            record('command', argv=[str(binary), '--open'], exit_code=result.returncode, stdout=result.stdout, stderr=result.stderr)
            assert result.returncode == 0
        history = calls()
        focuses = [call for call in history if len(call) > 1 and call[1] == 'focus']
        if expected_focus:
            assert focuses == [['workspace', 'focus', 'verify-w'], ['tab', 'focus', 'verify-t'], ['agent', 'focus', expected_focus]], focuses
        else:
            assert not focuses, focuses
        forbidden = [call for call in history if call[:2] == ['agent', 'read'] or call[:3] == ['terminal', 'session', 'observe']]
        assert not forbidden, forbidden
        if feature == 'launch':
            assert ['plugin', 'pane', 'open', '--plugin', 'yxhta.agents-picker', '--entrypoint', 'picker', '--focus'] in history
        snapshot('exited')
        (evidence / 'result.json').write_text(json.dumps({'feature': feature, 'case': case, 'passed': True, 'boundary': 'fixture Herdr; real release TUI'}))
    except BaseException as error:
        if started:
            snapshot('failure')
        (evidence / 'failure.json').write_text(json.dumps({'type': type(error).__name__, 'message': str(error)}))
        raise
    finally:
        if started:
            tmux('send-keys', '-t', 'verify:0.0', 'C-c', check=False)
            deadline = time.monotonic() + 3
            while not (scratch / 'exit.json').exists() and time.monotonic() < deadline:
                time.sleep(.1)
            tmux('kill-server', check=False)
        for name in ('calls.jsonl', 'fixture.json', 'config.toml', 'exit.json'):
            if (scratch / name).exists():
                shutil.copy2(scratch / name, evidence / name)
        server_remaining = False
        if started:
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                server_remaining = tmux('list-sessions', check=False).returncode == 0
                if not server_remaining:
                    break
                time.sleep(.1)
        record('cleanup', scratch=str(scratch), server_remaining=server_remaining)
        if server_remaining:
            raise RuntimeError(f'verification server still running at {socket}; scratch retained')
        shutil.rmtree(scratch)
        record("cleanup-proof", scratch_exists=scratch.exists(), socket_exists=socket.exists())
    print(evidence)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=['doctor', 'run'])
    parser.add_argument('feature', nargs='?', choices=['search', 'navigation', 'numbers', 'refresh', 'launch'])
    parser.add_argument('--binary', type=Path, default=BINARY)
    parser.add_argument('--evidence', type=Path)
    args = parser.parse_args()
    if args.command == 'doctor':
        print(json.dumps(doctor(args.binary.resolve()), indent=2))
    else:
        if not args.feature or not args.evidence:
            parser.error('run requires a feature and a new --evidence directory')
        if args.feature == 'numbers':
            args.evidence.mkdir(parents=True, exist_ok=False)
            cases = [f'digit-{digit}' for digit in '1234567890'] + [
                'short', 'empty', 'tiny', 'zero-height', 'modifiers', 'search-digits',
                'scroll', 'resize', 'reorder', 'selection', 'glyphs']
            for case in cases:
                run_case(args.feature, args.binary.resolve(), args.evidence.resolve() / case, case)
            (args.evidence / 'result.json').write_text(json.dumps({'passed': True, 'cases': cases}))
        else:
            run_case(args.feature, args.binary.resolve(), args.evidence.resolve())


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == 'fixture':
        sys.exit(fixture(sys.argv[2:]))
    if len(sys.argv) > 1 and sys.argv[1] == 'child':
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        code = subprocess.run([sys.argv[2]]).returncode
        (Path(os.environ['PICKER_VERIFY_SCRATCH']) / 'exit.json').write_text(json.dumps({'exit_code': code}))
        sys.exit(code)
    main()
