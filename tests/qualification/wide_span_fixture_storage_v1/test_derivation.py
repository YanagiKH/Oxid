"""Exact current discovery-test derivation; every predecessor equation survives."""

def exact(text,before,after):
    if text.count(before)!=1:raise ValueError("exact discovery-test seam")
    return text.replace(before,after)

def derive(original):
    s=original.decode();anchor='STREAMING_ROOT = '
    const='WIDE_FILES = tuple("fixtures/typed-parser-wide-span/" + name + ".ox" for name in (\n    "parser_main", "parser_state", "parser_signature", "parser_atom", "parser_call",\n    "parser_expression", "parser_statement", "parser_control", "parser_driver", "parser_output",\n    "control_main"))\nWIDE_ROOTS = ("parser_main", "control_main")\n\n'
    s=exact(s,anchor,const+anchor)
    anchor='        # The genuine streaming lexer is five new language files and one\n'
    change='        # The wide parser contributes exactly eleven visible language members.\n        # They enter only the separately materialized typed route, never legacy.\n        import build_wide_span_parser as wide\n        self.assertEqual(wide.OVERLAY_FILES, WIDE_FILES)\n        self.assertEqual(len(WIDE_FILES), 11)\n        self.assertEqual([name for name in language if name in WIDE_FILES], sorted(WIDE_FILES))\n        self.assertFalse(any(name in WIDE_FILES for name, _ in check_inventory + run_inventory))\n        wide_sources = verify_repo.wide_overlay_sources(root)\n        self.assertTrue({root / name for name in WIDE_FILES} <= {path for path, _ in wide_sources.values()})\n        self.assertEqual((len(language), len(check_inventory), len(run_inventory), typed_members, len(typed_entries)),\n                         (209 + 11, 143, 75, 79, 8))\n        language = [name for name in language if name not in WIDE_FILES]\n'
    s=exact(s,anchor,change+anchor)
    anchor='        predecessor_commands = [command for command in commands\n'
    change='        wide_commands = [command for command in commands if len(command) > 2\n                         and Path(command[2]).parent.name == "typed-parser-wide-span"]\n        self.assertEqual(len(wide_commands), 2)\n        self.assertEqual([Path(command[2]).stem for command in wide_commands], list(WIDE_ROOTS))\n        for command in wide_commands:\n            self.assertEqual(command[:2], [str(Path(sys.executable).resolve()), "check"])\n            self.assertEqual(command[3:], ["--edition=typed-preview"])\n            self.assertFalse(Path(command[2]).is_relative_to(verify_repo.ROOT))\n        self.assertFalse(any(str(verify_repo.ROOT / name) in command\n                             for name in WIDE_FILES for command in commands))\n        before_wide = [command for command in commands\n                       if command not in overlay_commands and not added_roots.intersection(command)]\n        self.assertEqual(len(before_wide), 205 + 2)\n        # Remove only the two validated typed entry commands, then retain every\n        # historical command/count/fingerprint assertion below.\n        commands = [command for command in commands if command not in wide_commands]\n'
    s=exact(s,anchor,change+anchor)
    s=exact(s, 'len(additional_members) + len(V2_OVERLAY_FILES)} language sources',
            'len(additional_members) + len(V2_OVERLAY_FILES) + len(WIDE_FILES)} language sources')
    s=exact(s, 'len(verify_repo.TYPED_CHECK_ONLY_PROJECTS) + len(V2_ROOTS)} checks',
            'len(verify_repo.TYPED_CHECK_ONLY_PROJECTS) + len(V2_ROOTS) + len(WIDE_ROOTS)} checks')
    anchor='        self.assertIn("9 v2 overlay source members / 2 materialized typed checks in a 31-module closure",'
    s=exact(s,anchor,'        self.assertIn("11 wide parser overlay members / 2 materialized typed checks", output.getvalue())\n'+anchor)
    return s.encode()
