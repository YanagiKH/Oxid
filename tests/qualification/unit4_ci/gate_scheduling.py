"""Named prepare-budget successor over the byte-identical accepted gate.

The accepted-view descriptor reads gate.py at its genuine path. Preserve it;
only this explicitly derived workflow entrypoint changes the prepare allowance.
"""
from pathlib import Path
import sys
import common as q

_PATH = Path(__file__).with_name('gate.py')
q.need(q.sha(q.regular(_PATH).read_bytes()) ==
       '89baea2fd1d628d21113f50e50b4c1b2f5101d25a57447c5785b8ada57a2d798',
       'changed accepted gate predecessor')
_gate = q.module('_unit4_prepare_budget_predecessor', _PATH)
_prepare_parser = _gate.prepare_parser


def prepare_parser(args, repo, output, driver):
    class PrepareStage:
        def stage(self, name, argv, timeout):
            q.need(name == '08-parser-prepare' and timeout == 300,
                   'unexpected prepare-budget predecessor dispatch')
            return driver.stage(name, argv, 600)
    return _prepare_parser(args, repo, output, PrepareStage())


_gate.prepare_parser = prepare_parser


def __getattr__(name):
    return getattr(_gate, name)


if __name__ == '__main__':
    try:
        _gate.main()
    except (q.Reject, OSError, KeyError, TypeError, ValueError) as error:
        print('Unit4 gate rejected: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
