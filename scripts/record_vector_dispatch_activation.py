"""Authenticate one protected host predicate evolution, without projecting inputs."""
import hashlib
import itertools

PATH = "tests/typed_project_dispatch.rs"
PREDECESSOR_PIN = (5888, "b24162962f6149422ebe38f58f6fe05d16de71ea06d81aa809df6b979035666d")
BEFORE = b'#[cfg(not(any(target_os = "linux", target_os = "macos")))]'
AFTER = b'''#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    all(
        target_os = "windows",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )
)))]'''


def pin(body):
    if type(body) is not bytes:
        raise ValueError("dispatch activation requires captured bytes")
    return len(body), hashlib.sha256(body).hexdigest()


def activation_difference():
    changed = []
    for os, arch, width, endian in itertools.product(
        ("linux", "macos", "windows", "other"),
        ("x86_64", "aarch64", "other"), ("64", "32"), ("little", "big")
    ):
        before = os not in ("linux", "macos")
        after = not (os in ("linux", "macos") or (
            os == "windows" and arch == "x86_64" and width == "64" and endian == "little"
        ))
        if before != after:
            changed.append({"os": os, "arch": arch, "pointer_width": width,
                            "endian": endian, "before": before, "after": after})
    return changed


def authenticate(predecessor, current, expected_pin, enabled):
    """Return proof metadata only; callers retain actual current bytes throughout."""
    if pin(predecessor) != PREDECESSOR_PIN:
        raise ValueError("wrong protected dispatch predecessor")
    if type(enabled) is not bool or pin(current) != expected_pin:
        raise ValueError("wrong pinned protected dispatch current input")
    if predecessor.count(BEFORE) != 1 or AFTER in predecessor:
        raise ValueError("protected dispatch predecessor seam drift")
    if enabled:
        if current.count(AFTER) != 1 or BEFORE in current:
            raise ValueError("protected dispatch activation seam drift")
        if current.replace(AFTER, BEFORE, 1) != predecessor:
            raise ValueError("protected dispatch inverse changes other bytes")
    elif current != predecessor:
        raise ValueError("protected dispatch evolution is not admitted")
    return {
        "version": "protected-dispatch-windows-activation-v1",
        "path": PATH, "activation_evolved": enabled,
        "predecessor": {"bytes": PREDECESSOR_PIN[0], "sha256": PREDECESSOR_PIN[1]},
        "current": {"bytes": len(current), "sha256": pin(current)[1]},
        "substitution_count": int(enabled),
        "substitution_before_sha256": hashlib.sha256(BEFORE).hexdigest(),
        "substitution_after_sha256": hashlib.sha256(AFTER).hexdigest(),
        "whole_file_inverse_equal": True,
        "predicate_equivalence_classes": 48,
        "changed_activations": activation_difference() if enabled else [],
        "materialized_input": "actual current bytes; inverse is proof metadata only",
        "windows_activation_qualified_by_this_gate": False,
    }
