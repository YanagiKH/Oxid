"""Additional specification-only token-concatenation controls; no compiler use."""
import unittest
from test_reviewer_owned_source import m, expectation


def borrowed_program(trivia,mutable=True,forwarded=False):
    b=m.Builder()
    read=m.Function('read',(('p','&mut S' if mutable else '&S'),),'i32',b.block(b.ret(b.f('p','value'))))
    main=m.Function('main',(),'i32',b.block(b.let('x',b.lit('S',('value',b.i(4))),True),
         b.ret(b.call('read',b.borrow('x',mutable,forwarded,trivia)))))
    return m.Program((m.Record('S',(('value','i32'),)),),(read,main))


class TokenBoundaryTests(unittest.TestCase):
    def test_preamble_line_comment_must_not_swallow_record(self):
        b=m.Builder();p=m.Program((m.Record('S'),),(m.Function('main',(),'i32',b.block(b.ret(b.i(7)))),),preamble='// hidden')
        with self.assertRaises(ValueError):m.render(p)

    def test_newline_trivia_line_comment_must_end_before_next_token(self):
        b=m.Builder();p=m.Program((m.Record('S'),),(m.Function('main',(),'i32',b.block(b.ret(b.i(7)))),),newline='// hidden')
        with self.assertRaises(ValueError):m.render(p)

    def test_borrow_line_comment_must_not_swallow_place(self):
        with self.assertRaises(ValueError):m.render(borrowed_program('// hidden'))

    def test_mutable_owner_borrow_keyword_requires_separation(self):
        with self.assertRaises(ValueError):m.render(borrowed_program(''))

    def test_valid_comment_token_boundaries_and_values(self):
        for trivia in ('/**/','//雪\n',' \r\n /*🦀*/ '):
            with self.subTest(trivia=trivia):
                self.assertEqual(expectation(borrowed_program(trivia))['expected']['result'],4)


if __name__=='__main__':unittest.main(verbosity=2)
