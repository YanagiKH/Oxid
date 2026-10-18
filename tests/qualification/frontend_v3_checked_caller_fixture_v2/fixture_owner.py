"""Private empty fixture namespace ownership; no general recursive deletion."""
from dataclasses import dataclass
import os
from pathlib import Path
import secrets
import tempfile
import physical_checks as pc


@dataclass(frozen=True)
class FixtureOwnerV2:
    schema: str
    ownership_token: str
    owned_root: str
    owned_root_identity: tuple
    parent_identity: tuple


_owners = {}


def create_owner(parent):
    pc.canonical(str(parent))
    anchor = pc.open_anchor(str(parent))
    try:
        root = tempfile.mkdtemp(prefix='fixture-owned-', dir=parent)
        owner = FixtureOwnerV2('oxid-v3-fixture-owner-v2', secrets.token_hex(16), root,
                               pc.directory_identity(os.stat(root, follow_symlinks=False)), anchor.identity)
        _owners[owner.ownership_token] = owner
        return owner
    finally:
        os.close(anchor.fd)


def validate(owner, source, evidence):
    pc.need(type(owner) is FixtureOwnerV2 and _owners.get(owner.ownership_token) is owner,
            'ownership unavailable')
    pc.need(owner.schema == 'oxid-v3-fixture-owner-v2', 'owner schema')
    paths = [Path(p) for p in (source, evidence, owner.owned_root)]
    for path in paths:
        pc.canonical(str(path))
    for i, first in enumerate(paths):
        for second in paths[i + 1:]:
            pc.need(first != second and first not in second.parents and second not in first.parents,
                    'fixture roots overlap')
    anchors, errors = [], []
    try:
        for path in paths:
            anchors.append(pc.open_anchor(str(path)))
        for i, first in enumerate(anchors):
            for j, second in enumerate(anchors):
                if i != j:
                    pc.need(first.identity[:2] not in {identity[:2] for _, identity in second.chain},
                            'physically overlapping roots')
        pc.need(anchors[2].identity == owner.owned_root_identity, 'owned root replaced')
        pc.need(anchors[2].chain[-2][1] == owner.parent_identity, 'owned parent replaced')
        for anchor in anchors:
            pc.check_anchor(anchor)
    except BaseException as error:
        errors.append(error)
    finally:
        for anchor in reversed(anchors):
            try:
                os.close(anchor.fd)
            except BaseException as error:
                errors.append(error)
    pc._finish(errors)


def cleanup(owner):
    pc.need(type(owner) is FixtureOwnerV2 and _owners.get(owner.ownership_token) is owner,
            'ownership unavailable')
    path = Path(owner.owned_root)
    parent = None
    child = None
    errors = []
    try:
        parent = pc.open_anchor(str(path.parent))
        pc.need(parent.identity == owner.parent_identity, 'owned parent replaced; left untouched')
        pc.need(pc.directory_identity(os.stat(path.name, dir_fd=parent.fd, follow_symlinks=False)) ==
                owner.owned_root_identity, 'owned root replaced; left untouched')
        child = os.open(path.name, pc.flags(True), dir_fd=parent.fd)
        pc.need(pc.directory_identity(os.fstat(child)) == owner.owned_root_identity, 'owned root changed')
        pc.need(os.listdir(child) == [], 'unexpected owned content; left untouched')
        pc.check_anchor(parent)
        pc.need(pc.directory_identity(os.stat(path.name, dir_fd=parent.fd, follow_symlinks=False)) ==
                owner.owned_root_identity, 'owned root changed; left untouched')
        os.rmdir(path.name, dir_fd=parent.fd)
    except BaseException as error:
        errors.append(error)
    finally:
        for fd in (child, parent.fd if parent is not None else None):
            if fd is not None:
                try:
                    os.close(fd)
                except BaseException as error:
                    errors.append(error)
        _owners.pop(owner.ownership_token, None)
    for error in errors:
        error.add_note('Fixture cleanup target (harness must inspect any leftover): ' + owner.owned_root)
    pc._finish(errors)
