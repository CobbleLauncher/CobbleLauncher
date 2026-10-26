# SPDX-License-Identifier: MIT
"""External WebDriver smoke of an unmodified packaged Linux production executable.

Requires Xvfb, xauth, dbus-run-session, WebKitWebDriver and tauri-driver on PATH.
Example: python3 scripts/linux-native-smoke.py --binary /tmp/deb/usr/bin/cobble-desktop
No Selenium, development server, embedded driver or production CSP changes.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import traceback
from urllib.request import Request, urlopen
from urllib.error import HTTPError


class FirstRunComplete(Exception):
    """Stop after first render while retaining the shared cleanup/report path."""


class GameUiComplete(Exception):
    """Stop the game-only workflow before unrelated Java and plugin checks."""


def is_active_owned_game(pid, game_directory):
    """Match the fixture's Minecraft JVM; a zombie is already exited."""
    process = Path(f'/proc/{pid}')
    try:
        command = (process / 'cmdline').read_bytes()
        stat = (process / 'stat').read_text(encoding='ascii')
        state = stat[stat.rfind(')') + 2:].split()[0]
    except (OSError, IndexError):
        return False
    return (state not in ('Z', 'X') and str(game_directory).encode() in command
            and b'net.minecraft.client.main.Main' in command)


def matching_game_process_ids(game_directory):
    result = []
    for process in Path('/proc').iterdir():
        if process.name.isdigit() and is_active_owned_game(int(process.name), game_directory):
            result.append(int(process.name))
    return result


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', type=Path, default=Path('validation/linux-native-smoke'))
    parser.add_argument('--require-java', action='store_true', help='Fail unless installed Java options are rendered')
    plugin_mode = parser.add_mutually_exclusive_group()
    plugin_mode.add_argument('--require-plugin-fallback', action='store_true', help='Require bundled Anvil Planner host-rendered report fallback')
    plugin_mode.add_argument('--require-plugin-frontend', action='store_true', help='Require bundled Anvil Planner native WebView frontend to open and close')
    parser.add_argument('--require-titlebar-edge', action='store_true', help='Require first-run window controls at the right edge')
    parser.add_argument('--first-run-only', action='store_true', help='Capture terms/titlebar without continuing onboarding')
    parser.add_argument('--game-ui-directory', type=Path, help='Exercise the packaged launcher against an already installed disposable game directory')
    parser.add_argument('--cache-dir', type=Path, help='Reuse an existing private XDG cache directory')
    args = parser.parse_args()
    if args.first_run_only and (args.require_java or args.require_plugin_fallback or args.require_plugin_frontend):
        parser.error('--first-run-only cannot require Java or a plugin mode')
    if sys.platform != 'linux':
        parser.error('Run on native Linux (including a Linux WSL distribution)')
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    for name in ('Xvfb', 'dbus-run-session', 'tauri-driver', 'WebKitWebDriver'):
        if not shutil.which(name):
            parser.error(f'Missing prerequisite: {name}')
    if args.require_plugin_frontend and not shutil.which('import'):
        parser.error('Missing prerequisite: ImageMagick import (install imagemagick)')
    if bool(args.game_ui_directory) != bool(args.cache_dir):
        parser.error('--game-ui-directory and --cache-dir must be supplied together')
    if args.game_ui_directory and (args.first_run_only or args.require_java
                                  or args.require_plugin_frontend or args.require_plugin_fallback):
        parser.error('--game-ui-directory runs only the game workflow; run other smoke modes separately')
    game_directory = args.game_ui_directory.resolve(strict=True) if args.game_ui_directory else None
    cache_directory = args.cache_dir.resolve(strict=True) if args.cache_dir else None
    report = {'result': 'FAIL', 'platform': platform.platform(), 'binary': str(binary),
              'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'checks': {},
              'scope': 'Production native WebView; isolated first run, Mesh off; no account or game launch' if not game_directory else 'Production native WebView; real UI game start, owned-process termination and UI recovery after exit. Normal Minecraft menu exit is covered by separate Core acceptance evidence.'}
    driver_binary = Path(shutil.which('tauri-driver')).resolve()
    report['driver'] = {'path': str(driver_binary), 'sha256': hashlib.sha256(driver_binary.read_bytes()).hexdigest()}
    install_record = driver_binary.parent.parent / '.crates.toml'
    if install_record.is_file():
        report['driver']['cargo_install_record'] = '\n'.join(
            line for line in install_record.read_text().splitlines() if 'tauri-driver ' in line)
    if shutil.which('dpkg-query'):
        packages = subprocess.run(['dpkg-query', '-W', '-f=${Package} ${Version}\n',
                                   'libwebkit2gtk-4.1-0', 'webkit2gtk-driver', 'xvfb'],
                                  capture_output=True, text=True, timeout=10, check=False)
        report['runtime_packages'] = packages.stdout.splitlines()
    owned = []
    logs = []
    session = None
    game_pid = None
    game_pids_before = None
    prefix = ''
    deadline = time.monotonic() + 180
    port, native_port = free_port(), free_port()
    while native_port == port:
        native_port = free_port()
    base = f'http://127.0.0.1:{port}'

    def request(method, route, payload=None):
        data = None if payload is None else json.dumps(payload).encode()
        req = Request(base + route, data=data, method=method, headers={'Content-Type': 'application/json'})
        try:
            with urlopen(req, timeout=min(30, max(1, deadline - time.monotonic()))) as response:
                value = json.load(response)['value']
        except HTTPError as error:
            raise RuntimeError(f'WebDriver {method} {route}: {error.read().decode(errors="replace")}') from error
        if isinstance(value, dict) and value.get('error'):
            raise RuntimeError(str(value))
        return value

    def execute(script, *values):
        return request('POST', prefix + '/execute/sync', {'script': script, 'args': list(values)})

    def wait(script, timeout=30):
        until = min(deadline, time.monotonic() + timeout)
        last_error = None
        while time.monotonic() < until:
            try:
                value = execute(script)
                if value:
                    return value
            except Exception as error:
                last_error = error
            time.sleep(.25)
        raise AssertionError(f'UI condition timed out: {script}; {last_error}')

    def click(selector, using='css selector'):
        element = request('POST', prefix + '/element', {'using': using, 'value': selector})
        element_id = element['element-6066-11e4-a52e-4f735466cecf']
        request('POST', prefix + f'/element/{element_id}/click', {})

    def click_css(selector):
        wait(f'return !!document.querySelector({json.dumps(selector)})')
        click(selector)

    def pointer_click(selector):
        def point():
            return execute("""const e=document.querySelector(arguments[0]); if(!e) return null;
                const r=e.getBoundingClientRect(), x=Math.round(r.left+r.width/2), y=Math.round(r.top+r.height/2);
                const hit=document.elementFromPoint(x,y);
                return {x,y,width:r.width,height:r.height,hit:hit?.outerHTML.slice(0,300),target:!!hit?.closest(arguments[0])};""", selector)
        wait(f"const e=document.querySelector({json.dumps(selector)}); if(!e) return false; const r=e.getBoundingClientRect(); return r.width>0 && r.height>0 && document.elementFromPoint(r.left+r.width/2,r.top+r.height/2)?.closest({json.dumps(selector)})===e", 15)
        geometry = point()
        request('POST', prefix + '/actions', {'actions': [{'type': 'pointer', 'id': 'mouse', 'parameters': {'pointerType': 'mouse'}, 'actions': [
            {'type': 'pointerMove', 'duration': 0, 'origin': 'viewport', 'x': geometry['x'], 'y': geometry['y']},
            {'type': 'pointerDown', 'button': 0}, {'type': 'pointerUp', 'button': 0}]}]})
        return geometry

    def screenshot(name):
        (output / name).write_bytes(base64.b64decode(request('GET', prefix + '/screenshot')))

    def launch(command, env, name):
        log = (output / name).open('wb')
        logs.append(log)
        process = subprocess.Popen(command, env=env, stdout=log, stderr=log, start_new_session=True)
        owned.append(process)
        return process

    with tempfile.TemporaryDirectory(prefix='cobble-linux-smoke-') as temporary:
        root = Path(temporary)
        env = dict(os.environ)
        for key, directory in {'HOME': 'home', 'XDG_CONFIG_HOME': 'config', 'XDG_DATA_HOME': 'data',
                               'XDG_CACHE_HOME': 'cache', 'XDG_STATE_HOME': 'state', 'XDG_RUNTIME_DIR': 'runtime'}.items():
            path = root / directory
            path.mkdir(mode=0o700)
            env[key] = str(path)
        if cache_directory:
            env['XDG_CACHE_HOME'] = str(cache_directory)
            options_file = game_directory / 'options.txt'
            option_text = options_file.read_text(encoding='utf-8')
            option_text, muted_count = re.subn(
                r'(?m)^soundCategory_(master|music):[^\r\n]*$',
                lambda match: f'soundCategory_{match.group(1)}:0.0', option_text)
            assert muted_count == 2, f'Expected master and music volume in fixture options.txt; found {muted_count}'
            options_file.write_text(option_text, encoding='utf-8')
            library = root / 'config' / 'CobbleLauncher' / 'games.json'
            library.parent.mkdir(parents=True, exist_ok=True)
            library.write_text(json.dumps([{
                'id': '09012110001', 'name': 'T090 UI Game', 'version': '1.21.1',
                'directory': str(game_directory), 'status': 'installed', 'error': None,
            }], indent=2) + '\n', encoding='utf-8')
            report['fixture'] = {'game_id': '09012110001', 'game_directory': str(game_directory),
                                 'library_file': str(library), 'cache_directory': str(cache_directory),
                                 'profile': 'created through the first-run offline profile UI',
                                 'audio': {'ALSOFT_DRIVERS': 'null',
                                           'soundCategory_master': 0.0, 'soundCategory_music': 0.0}}
        env.update({'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8', 'GDK_BACKEND': 'x11',
                    'LIBGL_ALWAYS_SOFTWARE': '1', 'ALSOFT_DRIVERS': 'null'})
        env.pop('WAYLAND_DISPLAY', None)
        env.pop('DBUS_SESSION_BUS_ADDRESS', None)
        # First-run defaults Mesh off; assert the persisted opt-out after UI setup.
        try:
            display_file = root / 'display'
            with display_file.open('wb') as display:
                xvfb_log = (output / 'xvfb.log').open('wb')
                logs.append(xvfb_log)
                xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(display.fileno()), '-screen', '0', '1280x900x24', '-nolisten', 'tcp'],
                                        env=env, pass_fds=(display.fileno(),), stdout=xvfb_log, stderr=xvfb_log, start_new_session=True)
                owned.append(xvfb)
                until = time.monotonic() + 10
                while not display_file.read_text().strip() and time.monotonic() < until:
                    if xvfb.poll() is not None:
                        raise RuntimeError('Xvfb exited')
                    time.sleep(.1)
            env['DISPLAY'] = ':' + display_file.read_text().strip()
            if env['DISPLAY'] == ':':
                raise RuntimeError('Xvfb did not allocate a display')
            driver = launch(['dbus-run-session', '--', 'tauri-driver', '--port', str(port), '--native-port', str(native_port)], env, 'driver.log')
            until = time.monotonic() + 20
            while True:
                try:
                    request('GET', '/status')
                    break
                except Exception:
                    if driver.poll() is not None or time.monotonic() > until:
                        raise RuntimeError('tauri-driver did not become ready')
                    time.sleep(.2)
            value = request('POST', '/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': str(binary)}}}})
            session = value['sessionId']
            prefix = f'/session/{session}'
            wait("return document.querySelector('.consent-page h1')?.textContent.trim()")
            wait("""return [...document.querySelectorAll('.window-controls button')].length === 3
                && [...document.querySelectorAll('.window-controls button')].every(e => {
                    const r=e.getBoundingClientRect(), s=getComputedStyle(e);
                    return r.width>0 && r.height>0 && s.display!=='none' && s.visibility==='visible' && +s.opacity>0;
                })""")
            # Let WebKit paint after the first-run view and titlebar have both mounted.
            time.sleep(.2)
            report['checks']['first_run_titlebar'] = execute("""const r=document.querySelector('.window-controls')?.getBoundingClientRect();
                return r?{viewport:innerWidth,left:r.left,right:r.right,remaining:innerWidth-r.right}:null""")
            if args.require_titlebar_edge:
                assert report['checks']['first_run_titlebar'] and report['checks']['first_run_titlebar']['remaining'] < 24, 'First-run window controls are not at the right edge'
            screenshot('01-terms.png')
            report['checks']['first_run_rendered'] = True
            if args.first_run_only:
                raise FirstRunComplete()
            click_css('.consent-check input')
            click_css('.consent-page .primary-button')
            wait("return !!document.querySelector('.setup-guide')")
            # Exercise persisted Core consent and each setup step; never enable Mesh.
            click_css('.guide-forward .primary-button')
            wait("return !!document.querySelector('.account-footer .text-button')")
            if game_directory:
                wait("return document.querySelectorAll('.account-setup .account-choice').length === 2", 20)
                click_css('.account-setup .account-choice-grid .account-choice:nth-child(2)')
                wait("return !!document.querySelector('#guided-player-name')", 20)
                element = request('POST', prefix + '/element', {'using': 'css selector', 'value': '#guided-player-name'})
                element_id = element['element-6066-11e4-a52e-4f735466cecf']
                request('POST', prefix + f'/element/{element_id}/value', {'text': 'CobbleProbe'})
                click_css('.account-footer button.primary-button')
                wait("return !!document.querySelector('.account-complete-scene')", 30)
                click_css('.account-footer button.primary-button')
                wait("return document.querySelector('.guide-count')?.textContent.trim().startsWith('3')", 30)
                report['checks']['offline_profile_created_through_ui'] = True
            else:
                click_css('.account-footer .text-button')
                wait("return document.querySelector('.guide-count')?.textContent.trim().startsWith('3')")
            click_css('.guide-forward .secondary-button')
            wait("return document.querySelector('.guide-count')?.textContent.trim().startsWith('4')")
            click_css('.guide-forward .secondary-button')
            wait("return !!document.querySelector('.main-navigation') && !document.querySelector('.first-run')")
            report['checks']['onboarding_core_persistence'] = True
            mesh_files = list((root / 'config').rglob('mesh-enabled.json'))
            report['checks']['mesh_persisted'] = [json.loads(path.read_text()) for path in mesh_files]
            assert mesh_files and all(json.loads(path.read_text()).get('enabled') is False for path in mesh_files), 'Mesh opt-out missing'
            screenshot('02-home.png')
            if game_directory:
                wait("const e=document.querySelector('.library-game-play'); return !!e && !e.disabled", 45)
                report['checks']['registered_real_game_visible'] = execute("const e=document.querySelector('.library-game'); return e?{id:e.dataset.gameId,status:e.dataset.gameStatus,label:e.getAttribute('aria-label')}:null")
                screenshot('03-game-ready.png')
                existing_game_pids = matching_game_process_ids(game_directory)
                assert not existing_game_pids, f'Refusing to start fixture with an existing Minecraft process: {existing_game_pids}'
                game_pids_before = set(existing_game_pids)
                launch_deadline = time.monotonic() + 35
                deadline = min(deadline, launch_deadline)
                click_css('.library-game-play')
                while game_pid is None and time.monotonic() < deadline:
                    new_game_pids = set(matching_game_process_ids(game_directory)) - game_pids_before
                    if new_game_pids:
                        game_pid = min(new_game_pids)
                        break
                    time.sleep(.25)
                assert game_pid, 'No new Minecraft process for the fixture appeared within the 35-second launch budget'
                report['checks']['owned_game_process'] = {'pid': game_pid, 'instance_directory_match': True}
                running = False
                while time.monotonic() < deadline:
                    state = execute("return document.querySelector('.running-workspace .session-status')?.dataset.state")
                    if state == 'running':
                        running = True
                        break
                    if not is_active_owned_game(game_pid, game_directory):
                        break
                    time.sleep(.25)
                assert running, f'Launcher UI did not reach running while owned process was active before the 35-second launch deadline'
                report['checks']['launcher_running_state'] = execute("return {heading:document.querySelector('.running-workspace h1')?.textContent.trim(),state:document.querySelector('.session-status')?.dataset.state}")
                screenshot('04-game-running.png')
                assert is_active_owned_game(game_pid, game_directory), 'Minecraft process identity changed before termination'
                report['checks']['owned_game_exit_method'] = 'SIGTERM to the exact PID after matching fixture directory and Minecraft main class; this does not test menu exit'
                # Bound process-exit observation and UI recovery together; cleanup retains time inside the 40-second limit.
                deadline = min(deadline, time.monotonic() + 30)
                os.kill(game_pid, signal.SIGTERM)
                outcome = None
                while time.monotonic() < deadline:
                    process_exited = not is_active_owned_game(game_pid, game_directory)
                    state = execute("return document.querySelector('.running-workspace .session-status')?.dataset.state")
                    if process_exited and state == 'closed':
                        play = execute("const b=document.querySelector('.running-workspace .session-status[data-state=closed] .primary-button'); return b?{label:b.textContent.trim(),available:!b.disabled}:null")
                        if play and play['available']:
                            outcome = {'state': 'closed', 'kind': 'play_available', 'action': play['label']}
                            break
                    if process_exited and state == 'failed':
                        retry = execute("const b=[...document.querySelectorAll('.launch-recovery .recovery-actions button')].find(e=>/try launch again|重新启动|retry/i.test(e.textContent)); return b?{label:b.textContent.trim(),available:!b.disabled}:null")
                        if retry and retry['available']:
                            outcome = {'state': 'failed', 'kind': 'retry_available_after_sigterm', 'action': retry['label']}
                            break
                    time.sleep(.25)
                assert outcome, f'Owned process exit did not produce a closed Play action or failed-state retry action before the 30-second deadline (active={is_active_owned_game(game_pid, game_directory)})'
                report['checks']['launcher_recovered_after_game_exit'] = {
                    **outcome,
                    'process_exited': not is_active_owned_game(game_pid, game_directory),
                    'normal_menu_exit_verified': False,
                }
                screenshot('05-ui-restored.png')
                raise GameUiComplete()
            click_css('.navigation-settings')
            wait("return document.querySelectorAll('[role=tab]').length > 0")
            click_css('[role="tab"][id$="trigger-runtime"]')
            wait("return !!document.querySelector('.settings-manual-java')")
            click_css('.settings-manual-java .settings-disclosure-trigger')
            wait("const e=document.querySelector('#detected-java'); return !!e && !e.disabled && e.getBoundingClientRect().height > 0", 45)
            report['checks']['java_control_geometry'] = pointer_click('#detected-java')
            wait("return document.querySelector('#detected-java')?.getAttribute('aria-expanded') === 'true'", 10)
            options = execute("return [...document.querySelectorAll('[role=option]')].map(e=>e.textContent.trim())")
            report['checks']['installed_java_options'] = options
            screenshot('03-java.png')
            report['checks']['ui_errors'] = execute("return [...document.querySelectorAll('[role=alert]')].map(e=>e.textContent.trim()).filter(Boolean)")
            assert not report['checks']['ui_errors'], 'UI reported errors'
            # Escape closes the Java list without selecting or changing a runtime.
            request('POST', prefix + '/actions', {'actions': [{'type': 'key', 'id': 'keyboard', 'actions': [
                {'type': 'keyDown', 'value': '\ue00c'}, {'type': 'keyUp', 'value': '\ue00c'}]}]})
            click_css('.main-sections button:nth-child(3)')
            wait("return !!document.querySelector('.tool-list')")
            report['checks']['bundled_tools_catalog'] = execute("return [...document.querySelectorAll('.tool-list button.tool-card')].map(e=>e.getAttribute('aria-label') || e.textContent.trim())")
            screenshot('04-tools.png')
            report['checks']['plugin_execution'] = 'Not exercised: no game fixture is installed; catalog render only'
            if args.require_plugin_fallback or args.require_plugin_frontend:
                click_css('.main-sections button:nth-child(4)')
                wait("return !!document.querySelector('.extensions-tabs')")
                click_css('.extensions-tabs [role="tab"][id$="trigger-discover"]')
                element = request('POST', prefix + '/element', {'using': 'css selector', 'value': '.extensions-search input'})
                element_id = element['element-6066-11e4-a52e-4f735466cecf']
                request('POST', prefix + f'/element/{element_id}/value', {'text': 'Anvil Planner'})
                wait("return document.querySelectorAll('.bundled-plugins > ul > li').length === 1")
                report['checks']['bundled_anvil_discovered'] = execute("return document.querySelector('.bundled-plugins > ul > li')?.textContent.trim()")
                click_css('.bundled-plugins > ul > li > button')
                wait("return !!document.querySelector('.plugin-approval')", 30)
                screenshot('05-plugin-review.png')
                click_css('.sensitive-permission')
                wait("return !!document.querySelector('.plugin-approval .primary-button:not(:disabled)')", 30)
                click_css('.plugin-approval .primary-button')
                wait("return !document.querySelector('.plugin-approval') && !!document.querySelector('.bundled-plugins')", 45)
                click_css('.extensions-tabs [role="tab"][id$="trigger-installed"]')
                wait("return !!document.querySelector('.plugin-list .plugin-identity-button')")
                report['checks']['installed_anvil'] = execute("return document.querySelector('.plugin-list li')?.textContent.trim()")
                screenshot('06-installed-plugin.png')
                click_css('.main-sections button:nth-child(3)')
                wait("return !!document.querySelector('.tool-search input')")
                if args.require_plugin_frontend:
                    execute("localStorage.removeItem('cobble.tools.usage.v1'); return true")
                element = request('POST', prefix + '/element', {'using': 'css selector', 'value': '.tool-search input'})
                element_id = element['element-6066-11e4-a52e-4f735466cecf']
                request('POST', prefix + f'/element/{element_id}/value', {'text': 'Anvil Planner'})
                click_css('button.tool-card[aria-label="Anvil Planner"]')
                if args.require_plugin_fallback:
                    wait("return !!document.querySelector('.plugin-report-panel .report-actions button:not(:disabled)')", 45)
                    report['checks']['bundled_plugin_host_fallback'] = {
                        'plugin': 'anvil-planner', 'panel': 'PluginReportPanel',
                        'actions': execute("return [...document.querySelectorAll('.plugin-report-panel .report-actions button')].map(e=>e.textContent.trim())"),
                        'scope': 'Native plugin catalog/manifest loading and isolation-safe host fallback; no Wasm execution or file dialog approval'}
                    screenshot('07-plugin-fallback.png')
                else:
                    ready = wait("""const e=document.querySelector('.plugin-web-view'); if(!e||e.getAttribute('aria-busy')!=='false'||e.querySelector('.loading-indicator,[role=alert]')||document.querySelector('.plugin-report-panel')) return false;
                        try { const d=JSON.parse(localStorage.getItem('cobble.tools.usage.v1')||'{}'); return d.entries?.some(x=>x.pluginId==='anvil-planner'&&x.commandId==='anvil-planner.open')||false; } catch { return false; }""", 60)
                    report['checks']['bundled_plugin_frontend_ready'] = {
                        'plugin': 'anvil-planner',
                        'host_state': execute("const e=document.querySelector('.plugin-web-view'); return e?{ariaBusy:e.getAttribute('aria-busy'),loading:!!e.querySelector('.loading-indicator'),error:!!e.querySelector('[role=alert]')}:null"),
                        'ready_event_recorded': ready,
                        'evidence': 'PluginWebView emits ready only after open_plugin_view returns; ToolsPage records the tool on that event.'}
                    frontend_image = output / '07-plugin-frontend-root.png'
                    # The open command acknowledges creation, not first paint in
                    # the separate WebKit process. The root capture is reviewed
                    # for actual tool content; an empty surface is not acceptance.
                    time.sleep(2)
                    subprocess.run(['import', '-window', 'root', str(frontend_image)], env=env, check=True, timeout=30)
                    report['checks']['plugin_frontend_root_capture'] = {
                        'path': frontend_image.name,
                        'bytes': frontend_image.stat().st_size,
                        'sha256': hashlib.sha256(frontend_image.read_bytes()).hexdigest()}
                    assert frontend_image.stat().st_size > 0, 'ImageMagick produced an empty root capture'
                    click_css('.navigation-settings')
                    wait("return !document.querySelector('.plugin-web-view') && document.querySelectorAll('[role=tab]').length > 0", 30)
                    click_css('[role="tab"]')
                    report['checks']['plugin_child_closed_main_interactive'] = {
                        'plugin_webview_host_unmounted': True,
                        'settings_tab_clicked': execute("return !!document.querySelector('[role=tab][aria-selected=true]')")}
                    assert report['checks']['plugin_child_closed_main_interactive']['settings_tab_clicked'], 'Main settings view did not remain interactive after closing plugin frontend'
                    closed_image = output / '08-plugin-frontend-closed-root.png'
                    subprocess.run(['import', '-window', 'root', str(closed_image)], env=env, check=True, timeout=30)
                    report['checks']['plugin_frontend_closed_root_capture'] = {
                        'path': closed_image.name,
                        'bytes': closed_image.stat().st_size,
                        'sha256': hashlib.sha256(closed_image.read_bytes()).hexdigest()}
                    assert closed_image.stat().st_size > 0, 'ImageMagick produced an empty post-navigation root capture'
            if args.require_java:
                assert options, 'No installed Java was rendered by native Core discovery'
            report['result'] = 'PASS'
        except FirstRunComplete:
            report['result'] = 'PASS'
        except GameUiComplete:
            report['result'] = 'PASS'
        except Exception as error:
            report['error'] = f'{type(error).__name__}: {error}'
            report['traceback'] = traceback.format_exc()
            if session:
                try:
                    screenshot('failure.png')
                    (output / 'failure-dom.html').write_text(execute('return document.documentElement.outerHTML'), encoding='utf-8')
                except Exception:
                    pass
        finally:
            if game_directory and game_pids_before is not None:
                cleanup_pids = ([game_pid] if game_pid is not None else
                                sorted(set(matching_game_process_ids(game_directory)) - game_pids_before))
                for owned_pid in cleanup_pids:
                    if is_active_owned_game(owned_pid, game_directory):
                        try:
                            os.kill(owned_pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                        until = time.monotonic() + 3
                        while is_active_owned_game(owned_pid, game_directory) and time.monotonic() < until:
                            time.sleep(.1)
                        if is_active_owned_game(owned_pid, game_directory):
                            try:
                                os.kill(owned_pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                if game_pid is not None:
                    report['owned_game_reaped'] = not is_active_owned_game(game_pid, game_directory)
                else:
                    report['new_owned_game_pids_reaped'] = all(
                        not is_active_owned_game(owned_pid, game_directory) for owned_pid in cleanup_pids)
            if session:
                try:
                    request('DELETE', prefix)
                except Exception:
                    pass
            # Only terminate process groups created by this harness, never name-based kills.
            for process in reversed(owned):
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
            report['owned_processes_reaped'] = all(process.poll() is not None for process in owned)
            for log in logs:
                log.close()
    (output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report['result'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
