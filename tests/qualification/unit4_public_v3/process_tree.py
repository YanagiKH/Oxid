#!/usr/bin/env python3
"""Portable process-tree ownership for bounded qualification subprocesses."""
import sys
sys.dont_write_bytecode = True
import ctypes
import errno
import os
import signal
import subprocess

class ProcessTree:
    def __init__(self, argv, cwd, env, new_session=True):
        self.child = None
        self.job = None
        self.group = None
        self.owned_group = new_session
        if os.name != 'nt':
            self.child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=new_session)
            self.group = self.child.pid if new_session else os.getpgrp()
            return
        # A suspended Windows child enters a kill-on-close Job Object before any
        # candidate instructions execute. Descendants inherit the job membership.
        from ctypes import wintypes as w
        k = ctypes.WinDLL('kernel32', use_last_error=True)
        self.kernel = k
        class BASIC(ctypes.Structure):
            _fields_ = [('PerProcessUserTimeLimit', ctypes.c_longlong), ('PerJobUserTimeLimit', ctypes.c_longlong),
                        ('LimitFlags', w.DWORD), ('MinimumWorkingSetSize', ctypes.c_size_t), ('MaximumWorkingSetSize', ctypes.c_size_t),
                        ('ActiveProcessLimit', w.DWORD), ('Affinity', ctypes.c_size_t), ('PriorityClass', w.DWORD), ('SchedulingClass', w.DWORD)]
        class IO(ctypes.Structure):
            _fields_ = [(name, ctypes.c_ulonglong) for name in ('ReadOperationCount', 'WriteOperationCount', 'OtherOperationCount', 'ReadTransferCount', 'WriteTransferCount', 'OtherTransferCount')]
        class EXTENDED(ctypes.Structure):
            _fields_ = [('BasicLimitInformation', BASIC), ('IoInfo', IO), ('ProcessMemoryLimit', ctypes.c_size_t),
                        ('JobMemoryLimit', ctypes.c_size_t), ('PeakProcessMemoryUsed', ctypes.c_size_t), ('PeakJobMemoryUsed', ctypes.c_size_t)]
        class THREADENTRY(ctypes.Structure):
            _fields_ = [('dwSize', w.DWORD), ('cntUsage', w.DWORD), ('th32ThreadID', w.DWORD), ('th32OwnerProcessID', w.DWORD),
                        ('tpBasePri', w.LONG), ('tpDeltaPri', w.LONG), ('dwFlags', w.DWORD)]
        signatures = {
            'CreateJobObjectW': ([ctypes.c_void_p, w.LPCWSTR], w.HANDLE),
            'SetInformationJobObject': ([w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD], w.BOOL),
            'OpenProcess': ([w.DWORD, w.BOOL, w.DWORD], w.HANDLE),
            'AssignProcessToJobObject': ([w.HANDLE, w.HANDLE], w.BOOL),
            'CreateToolhelp32Snapshot': ([w.DWORD, w.DWORD], w.HANDLE),
            'Thread32First': ([w.HANDLE, ctypes.POINTER(THREADENTRY)], w.BOOL),
            'Thread32Next': ([w.HANDLE, ctypes.POINTER(THREADENTRY)], w.BOOL),
            'OpenThread': ([w.DWORD, w.BOOL, w.DWORD], w.HANDLE),
            'ResumeThread': ([w.HANDLE], w.DWORD),
            'TerminateJobObject': ([w.HANDLE, w.UINT], w.BOOL),
            'CloseHandle': ([w.HANDLE], w.BOOL),
        }
        for name, (args, result) in signatures.items():
            function = getattr(k, name)
            function.argtypes, function.restype = args, result
        def checked(value):
            if not value:
                raise ctypes.WinError(ctypes.get_last_error())
            return value
        self.job = checked(k.CreateJobObjectW(None, None))
        limits = EXTENDED()
        limits.BasicLimitInformation.LimitFlags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        try:
            checked(k.SetInformationJobObject(self.job, 9, ctypes.byref(limits), ctypes.sizeof(limits)))
            self.child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                          creationflags=subprocess.CREATE_NEW_PROCESS_GROUP | 0x4)  # CREATE_SUSPENDED
            process_handle = checked(k.OpenProcess(0x0100 | 0x0001, False, self.child.pid))
            try:
                checked(k.AssignProcessToJobObject(self.job, process_handle))
            finally:
                checked(k.CloseHandle(process_handle))
            snapshot = k.CreateToolhelp32Snapshot(0x4, 0)  # TH32CS_SNAPTHREAD
            if snapshot == ctypes.c_void_p(-1).value:
                raise ctypes.WinError(ctypes.get_last_error())
            try:
                entry = THREADENTRY()
                entry.dwSize = ctypes.sizeof(entry)
                found = 0
                exists = k.Thread32First(snapshot, ctypes.byref(entry))
                while exists:
                    if entry.th32OwnerProcessID == self.child.pid:
                        thread = checked(k.OpenThread(0x0002, False, entry.th32ThreadID))
                        try:
                            if k.ResumeThread(thread) == 0xffffffff:
                                raise ctypes.WinError(ctypes.get_last_error())
                            found += 1
                        finally:
                            checked(k.CloseHandle(thread))
                    exists = k.Thread32Next(snapshot, ctypes.byref(entry))
                if found != 1:
                    raise RuntimeError('expected exactly one suspended initial process thread')
            finally:
                checked(k.CloseHandle(snapshot))
        except BaseException:
            if self.child:
                self.child.kill()
                self.child.wait()
            self.close()
            raise

    def kill(self):
        if os.name == 'nt':
            if not self.kernel.TerminateJobObject(self.job, 125):
                raise ctypes.WinError(ctypes.get_last_error())
        else:
            try:
                os.killpg(self.group, signal.SIGKILL)
            except OSError as error:
                if error.errno != errno.ESRCH:
                    raise

    def close(self):
        if os.name != 'nt' and self.owned_group and self.group is not None:
            self.kill()
            self.group = None
        if self.job is not None:
            if not self.kernel.CloseHandle(self.job):
                raise ctypes.WinError(ctypes.get_last_error())
            self.job = None
