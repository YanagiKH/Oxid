"""Test-only atomic Windows job ownership with regular-file transcripts."""
import ctypes
from ctypes import wintypes as W
import os
from functools import cmp_to_key
from pathlib import Path
import subprocess
import time


class Security(ctypes.Structure):
    _fields_ = [("length", W.DWORD), ("descriptor", W.LPVOID), ("inherit", W.BOOL)]


class Startup(ctypes.Structure):
    _fields_ = [("cb", W.DWORD), ("reserved", W.LPWSTR), ("desktop", W.LPWSTR),
                ("title", W.LPWSTR), ("x", W.DWORD), ("y", W.DWORD),
                ("width", W.DWORD), ("height", W.DWORD), ("chars_x", W.DWORD),
                ("chars_y", W.DWORD), ("fill", W.DWORD), ("flags", W.DWORD),
                ("show", W.WORD), ("reserved_bytes", W.WORD), ("data", W.LPVOID),
                ("stdin", W.HANDLE), ("stdout", W.HANDLE), ("stderr", W.HANDLE)]


class StartupEx(ctypes.Structure):
    _fields_ = [("startup", Startup), ("attributes", W.LPVOID)]


class Information(ctypes.Structure):
    _fields_ = [("process", W.HANDLE), ("thread", W.HANDLE),
                ("pid", W.DWORD), ("tid", W.DWORD)]


class BasicLimits(ctypes.Structure):
    _fields_ = [("process_time", ctypes.c_int64), ("job_time", ctypes.c_int64),
                ("flags", W.DWORD), ("minimum", ctypes.c_size_t),
                ("maximum", ctypes.c_size_t), ("process_limit", W.DWORD),
                ("affinity", ctypes.c_size_t), ("priority", W.DWORD),
                ("scheduling", W.DWORD)]


class Limits(ctypes.Structure):
    _fields_ = [("basic", BasicLimits), ("io", ctypes.c_uint64 * 6),
                ("process_memory", ctypes.c_size_t), ("job_memory", ctypes.c_size_t),
                ("peak_process", ctypes.c_size_t), ("peak_job", ctypes.c_size_t)]


def kernel():
    if os.name != "nt" or ctypes.sizeof(ctypes.c_void_p) != 8:
        raise ValueError("requires an actual 64-bit Windows process")
    api = ctypes.WinDLL("kernel32", use_last_error=True)
    signatures = {
        "CreateJobObjectW": ([W.LPVOID, W.LPCWSTR], W.HANDLE),
        "SetInformationJobObject": ([W.HANDLE, ctypes.c_int, W.LPVOID, W.DWORD], W.BOOL),
        "QueryInformationJobObject": ([W.HANDLE, ctypes.c_int, W.LPVOID, W.DWORD, W.LPVOID], W.BOOL),
        "TerminateJobObject": ([W.HANDLE, W.UINT], W.BOOL),
        "CloseHandle": ([W.HANDLE], W.BOOL),
        "InitializeProcThreadAttributeList": ([W.LPVOID, W.DWORD, W.DWORD, ctypes.POINTER(ctypes.c_size_t)], W.BOOL),
        "UpdateProcThreadAttribute": ([W.LPVOID, W.DWORD, ctypes.c_size_t, W.LPVOID,
                                       ctypes.c_size_t, W.LPVOID, W.LPVOID], W.BOOL),
        "DeleteProcThreadAttributeList": ([W.LPVOID], None),
        "CreateProcessW": ([W.LPCWSTR, W.LPWSTR, W.LPVOID, W.LPVOID, W.BOOL,
                            W.DWORD, W.LPVOID, W.LPCWSTR, ctypes.POINTER(StartupEx),
                            ctypes.POINTER(Information)], W.BOOL),
        "WaitForSingleObject": ([W.HANDLE, W.DWORD], W.DWORD),
        "GetExitCodeProcess": ([W.HANDLE, ctypes.POINTER(W.DWORD)], W.BOOL),
        "OpenProcess": ([W.DWORD, W.BOOL, W.DWORD], W.HANDLE),
        "CompareStringOrdinal": ([W.LPCWSTR, ctypes.c_int, W.LPCWSTR, ctypes.c_int, W.BOOL], ctypes.c_int),
    }
    for name, (args, result) in signatures.items():
        function = getattr(api, name)
        function.argtypes, function.restype = args, result
    return api


def checked(result, message):
    if not result:
        raise OSError(ctypes.get_last_error(), message)
    return result


def run(argv, cwd, environment, stdout, stderr, timeout=300, stdin=None, tick=None):
    """No spawn/assign race: JOB_LIST is applied inside CreateProcessW.

    Files avoid pipe-reader scheduling in this independent test orchestrator.
    The owned job bounds memory and all descendants. Transcripts are limited to
    8 MiB each, independently of the production doctor's 64 KiB pipe contract.
    """
    import msvcrt
    api = kernel()
    assert ctypes.sizeof(Security) == 24
    assert ctypes.sizeof(StartupEx) == 112 and StartupEx.attributes.offset == 104
    assert ctypes.sizeof(Information) == 24 and ctypes.sizeof(Limits) == 144
    command = subprocess.list2cmdline(list(map(str, argv)))
    if len(command.encode("utf-16-le")) // 2 >= 32767:
        raise ValueError("test command exceeds Windows command length")
    for key, value in environment.items():
        if "\0" in key + value or "=" in key:
            raise ValueError("invalid test environment entry")
    def compare(left, right):
        order = api.CompareStringOrdinal(left[0], -1, right[0], -1, True)
        if order not in (1, 2, 3):
            raise OSError(ctypes.get_last_error(), "compare qualification environment names")
        return order - 2

    block = "\0".join(k + "=" + v for k, v in sorted(
        environment.items(), key=cmp_to_key(compare))) + "\0\0"
    env = ctypes.create_unicode_buffer(block)
    command = ctypes.create_unicode_buffer(command)
    job = checked(api.CreateJobObjectW(None, None), "create qualification job")
    process = thread = attributes = None
    handles = []
    started = time.monotonic()
    stdout, stderr = Path(stdout), Path(stderr)
    try:
        limits = Limits()
        limits.basic.flags = 0x2000 | 0x200  # KILL_ON_JOB_CLOSE | JOB_MEMORY
        limits.job_memory = 1024 * 1024 * 1024
        checked(api.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)),
                "set qualification job limits")
        handles = [open(os.devnull if stdin is None else stdin, "rb"),
                   stdout.open("xb"), stderr.open("xb")]
        native = (W.HANDLE * 3)(*(msvcrt.get_osfhandle(file.fileno()) for file in handles))
        for handle in native:
            os.set_handle_inheritable(handle, True)
        jobs = (W.HANDLE * 1)(job)
        size = ctypes.c_size_t()
        result = api.InitializeProcThreadAttributeList(None, 2, 0, ctypes.byref(size))
        if result or ctypes.get_last_error() != 122 or not 0 < size.value <= 4096:
            raise OSError("invalid qualification attribute size")
        storage = ctypes.create_string_buffer(size.value + 15)
        raw_attributes = ctypes.c_void_p((ctypes.addressof(storage) + 15) & ~15)
        checked(api.InitializeProcThreadAttributeList(raw_attributes, 2, 0, ctypes.byref(size)),
                "initialize qualification attributes")
        attributes = raw_attributes
        for key, values in ((0x20002, native), (0x2000D, jobs)):
            checked(api.UpdateProcThreadAttribute(attributes, 0, key, values,
                                                  ctypes.sizeof(values), None, None),
                    "set atomic qualification attributes")
        startup = StartupEx()
        startup.startup.cb = ctypes.sizeof(startup)
        startup.startup.flags = 0x100
        startup.startup.stdin, startup.startup.stdout, startup.startup.stderr = native
        startup.attributes = attributes
        information = Information()
        checked(api.CreateProcessW(str(Path(argv[0]).resolve(strict=True)), command, None, None,
                                   True, 0x80000 | 0x400 | 0x8000000, env, str(cwd),
                                   ctypes.byref(startup), ctypes.byref(information)),
                "create atomically job-bound qualification process")
        process, thread = information.process, information.thread
        api.CloseHandle(thread)
        thread = None
        api.DeleteProcThreadAttributeList(attributes)
        attributes = None
        for handle in native:
            os.set_handle_inheritable(handle, False)
        for file in handles:
            file.close()
        handles = []
        while True:
            if tick is not None:
                tick()
            if stdout.stat().st_size > 8 * 1024 * 1024 or stderr.stat().st_size > 8 * 1024 * 1024:
                raise ValueError("qualification transcript exceeded 8 MiB per stream")
            wait = api.WaitForSingleObject(process, 0)
            if wait == 0:
                code = W.DWORD()
                checked(api.GetExitCodeProcess(process, ctypes.byref(code)), "read qualification status")
                return code.value, time.monotonic() - started
            if wait != 258:
                raise OSError(ctypes.get_last_error(), "wait for qualification process")
            if time.monotonic() - started >= timeout:
                raise TimeoutError("qualification process exceeded its deadline")
            time.sleep(0.005)
    finally:
        try:
            checked(api.TerminateJobObject(job, 1), "terminate qualification process tree")
            deadline = time.monotonic() + 2
            while process:
                accounting = (ctypes.c_uint64 * 6)()
                checked(api.QueryInformationJobObject(job, 1, accounting, ctypes.sizeof(accounting), None),
                        "query qualification job completion")
                if api.WaitForSingleObject(process, 0) == 0 and accounting[5] & 0xFFFFFFFF == 0:
                    break
                if time.monotonic() >= deadline:
                    raise TimeoutError("qualification process tree did not terminate")
                time.sleep(0.005)
        finally:
            if attributes:
                api.DeleteProcThreadAttributeList(attributes)
            for file in handles:
                file.close()
            for handle in (thread, process, job):
                if handle:
                    api.CloseHandle(handle)
