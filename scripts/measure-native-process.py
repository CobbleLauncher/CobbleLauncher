# SPDX-License-Identifier: MIT
"""Windows, standard-library-only process-tree sampler; never launches or kills.

Attach to an owned test launcher or a launcher the user explicitly opened. No argv, environment,
credential files, or application data are read. Private working set counts
resident non-shared pages from QueryWorkingSet; PrivateUsage is separate.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import json
import os
from pathlib import Path
import statistics
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--pid', type=int, required=True)
    parser.add_argument('--seconds', type=float, default=30)
    parser.add_argument('--interval', type=float, default=1)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--scenario', default='idle, user-opened process')
    args = parser.parse_args()
    if os.name != 'nt':
        raise SystemExit('Windows only')
    kernel, psapi = c.WinDLL('kernel32', use_last_error=True), c.WinDLL('psapi', use_last_error=True)
    pointer = c.c_size_t

    class Entry(c.Structure):
        _fields_ = [('size', w.DWORD), ('usage', w.DWORD), ('pid', w.DWORD),
                    ('heap', pointer), ('module', w.DWORD), ('threads', w.DWORD),
                    ('parent', w.DWORD), ('priority', w.LONG), ('flags', w.DWORD),
                    ('name', w.WCHAR * 260)]

    class Memory(c.Structure):
        _fields_ = [('size', w.DWORD), ('faults', w.DWORD)] + [(name, pointer) for name in
                    ('peak_ws', 'ws', 'peak_paged', 'paged', 'peak_nonpaged', 'nonpaged', 'pagefile', 'peak_pagefile', 'private_bytes')]

    kernel.OpenProcess.argtypes, kernel.OpenProcess.restype = [w.DWORD, w.BOOL, w.DWORD], w.HANDLE
    kernel.CloseHandle.argtypes = [w.HANDLE]
    kernel.CreateToolhelp32Snapshot.argtypes, kernel.CreateToolhelp32Snapshot.restype = [w.DWORD, w.DWORD], w.HANDLE
    kernel.Process32FirstW.argtypes = kernel.Process32NextW.argtypes = [w.HANDLE, c.POINTER(Entry)]
    kernel.GetProcessTimes.argtypes = [w.HANDLE] + [c.POINTER(w.FILETIME)] * 4
    psapi.GetProcessMemoryInfo.argtypes = [w.HANDLE, c.POINTER(Memory), w.DWORD]
    psapi.QueryWorkingSet.argtypes = [w.HANDLE, c.c_void_p, w.DWORD]

    def ticks(filetime):
        return (filetime.dwHighDateTime << 32) | filetime.dwLowDateTime

    def sample(pid):
        handle = kernel.OpenProcess(0x0400 | 0x0010, False, pid)
        if not handle:
            return {'pid': pid, 'unavailable': c.get_last_error()}
        try:
            times = [w.FILETIME() for _ in range(4)]
            if not kernel.GetProcessTimes(handle, *[c.byref(item) for item in times]):
                return {'pid': pid, 'unavailable': c.get_last_error()}
            memory = Memory(size=c.sizeof(Memory))
            if not psapi.GetProcessMemoryInfo(handle, c.byref(memory), c.sizeof(memory)):
                return {'pid': pid, 'unavailable': c.get_last_error()}
            # The structure begins with NumberOfEntries followed by one ULONG_PTR
            # per resident page. Bit 8 is Shared in PSAPI_WORKING_SET_BLOCK.
            capacity = max(16384, memory.ws // 4096 + 4096)
            private_ws = None
            for _ in range(4):
                buffer = (pointer * (capacity + 1))()
                if psapi.QueryWorkingSet(handle, buffer, c.sizeof(buffer)):
                    count = buffer[0]
                    private_ws = sum(not (buffer[index] & (1 << 8)) for index in range(1, count + 1)) * 4096
                    break
                capacity *= 2
            return {'pid': pid, 'created_ticks': ticks(times[0]),
                    'cpu_seconds': (ticks(times[2]) + ticks(times[3])) / 10_000_000,
                    'working_set_bytes': memory.ws, 'private_bytes': memory.private_bytes,
                    'private_working_set_bytes': private_ws}
        finally:
            kernel.CloseHandle(handle)

    def tree():
        snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
        if snapshot == c.c_void_p(-1).value:
            raise OSError(c.get_last_error(), 'process snapshot failed')
        try:
            entry = Entry(size=c.sizeof(Entry))
            parents, names = {}, {}
            more = kernel.Process32FirstW(snapshot, c.byref(entry))
            while more:
                parents[entry.pid] = entry.parent
                names[entry.pid] = entry.name
                more = kernel.Process32NextW(snapshot, c.byref(entry))
            owned = {args.pid}
            while True:
                children = {pid for pid, parent in parents.items() if parent in owned}
                if children <= owned:
                    return {pid: names.get(pid, '') for pid in owned}
                owned |= children
        finally:
            kernel.CloseHandle(snapshot)

    identity = sample(args.pid)
    if 'unavailable' in identity:
        raise SystemExit(f"Requested process unavailable: {identity['unavailable']}")
    rows, last_cpu = [], {}
    started = previous = time.perf_counter()
    previous_wall_ticks = int((time.time() + 11644473600) * 10_000_000)
    while time.perf_counter() - started < args.seconds:
        now = time.perf_counter()
        current_root = sample(args.pid)
        if current_root.get('created_ticks') != identity['created_ticks']:
            break
        processes = [{**sample(pid), 'image_name': name} for pid, name in sorted(tree().items())]
        processes = [item for item in processes if 'unavailable' in item or item['created_ticks'] >= identity['created_ticks']]
        measured = [item for item in processes if 'cpu_seconds' in item]
        delta_cpu = 0.0
        for item in measured:
            key = (item['pid'], item['created_ticks'])
            if key in last_cpu:
                delta_cpu += max(0, item['cpu_seconds'] - last_cpu[key])
            elif rows and item['created_ticks'] >= previous_wall_ticks:
                delta_cpu += item['cpu_seconds']
            last_cpu[key] = item['cpu_seconds']
        complete = len(measured) == len(processes) and all(item['private_working_set_bytes'] is not None for item in measured)
        rows.append({'elapsed_seconds': now - started, 'process_count': len(processes), 'complete': complete,
                     'tree_private_working_set_bytes': sum(item['private_working_set_bytes'] or 0 for item in measured) if complete else None,
                     'tree_private_bytes': sum(item['private_bytes'] for item in measured),
                     'cpu_percent_one_core': delta_cpu / (now - previous) * 100 if rows else None,
                     'processes': processes})
        previous = now
        previous_wall_ticks = int((time.time() + 11644473600) * 10_000_000)
        time.sleep(max(0.01, args.interval - (time.perf_counter() - now)))
    private = [row['tree_private_working_set_bytes'] for row in rows if row['complete']]
    report = {'scenario': args.scenario, 'scope': 'selected native process and sampled descendants',
              'cache_condition': 'uncontrolled; cannot classify cold or warm launch',
              'process_to_home_ready_ms': None, 'startup_unavailable_reason': 'Attach-only sampler has no owned process creation or native home-ready marker.',
              'method': 'Toolhelp PID/parent tree; QueryWorkingSet non-shared resident pages; GetProcessTimes CPU deltas. No command lines read. No processes launched or killed.',
              'limitations': 'Children exiting between samples are absent. First sample has no CPU delta. Sampler overhead is excluded from target tree. One run is not P95.',
              'private_working_set_median_bytes': statistics.median(private) if private else None,
              'private_working_set_peak_bytes': max(private) if private else None, 'samples': rows}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({key: value for key, value in report.items() if key != 'samples'}, indent=2))


if __name__ == '__main__':
    main()
