import importlib.util
import unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('converter',Path(__file__).resolve().parents[2]/'scripts/check/convert-book-chapter.py')
m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
class ConverterTests(unittest.TestCase):
    def fixture(self, pages):
        return 'use super::{Primary, Route, Sub};\nconst CELL: char = \'⠡\';\nconst X: Route = Route::new(CELL, \'⠁\');\nconst PRIMARY: Primary = Primary { subs: &[Sub { route: X, pages: &['+pages+'] }] };\n'
    def test_multiline_unicode_and_literal_escapes_roundtrip(self):
        source=self.fixture(r'"one\nline\n", "tab\tend", "unicode \u{1f980}", "quoted \"value\"", "nul\0end"')
        code,data,records=m.convert(source,'fixture')
        self.assertEqual([p for _,p in records],['one\nline\n','tab\tend','unicode 🦀','quoted "value"','nul\0end'])
        self.assertEqual(m.decode(data),records)
        self.assertIn("catalog_pages::<5>(CATALOG, b'1')",code)
    def test_rust_continuation_trims_indentation(self):
        _,_,records=m.convert(self.fixture('"first\\\n    next"'),'fixture')
        self.assertEqual(records[0][1],'firstnext')
    def test_rejects_dynamic_and_delimiter_collision(self):
        with self.assertRaises(m.Unsupported): m.convert(self.fixture('SOME_CONSTANT'),'fixture')
        with self.assertRaises(m.Unsupported): m.convert(self.fixture('"@@book-page 1@@"'),'fixture')
        with self.assertRaises(m.Unsupported): m.convert(self.fixture('"ok"'),'d3456_advisories')
    def test_existing_catalog_route_keeps_its_name(self):
        source=self.fixture('"page"').replace('const X: Route', 'const CATALOG: Route').replace('route: X', 'route: CATALOG')
        code,_,_=m.convert(source,'fixture')
        self.assertIn('const CATALOG: Route', code)
        self.assertIn('const BOOK_PAGES_CATALOG: &str', code)
        self.assertIn('catalog_pages::<1>(BOOK_PAGES_CATALOG', code)
    def test_supports_additional_imports_without_changing_them(self):
        source=self.fixture('"page"').replace('use super::{Primary, Route, Sub};', 'use super::{Primary, Raise, Route, Sub};')
        code,_,_=m.convert(source,'fixture')
        self.assertIn('use super::{Primary, Raise, Route, Sub};', code)
        self.assertIn('const CATALOG: &str', code)
    def test_multishelf_keeps_identical_digits_separate(self):
        source = '''use super::{Primary, Route, Sub};
const A: Route = Route::new(CELL, '⠁');
const B: Route = Route::new(SHELF_II_CELL, '⠁');
const C: Route = Route::new(SHELF_III_CELL, '⠁');
const D: Route = Route::new(CONTEXT_CELL, '⠁');
Sub { route: A, pages: &["one\\nline"] }
Sub { route: B, pages: &["two"] }
Sub { route: C, pages: &["three"] }
Sub { route: D, pages: &["four"] }
'''
        code, assets, records = m.convert_codex_shelves(source)
        self.assertEqual(records['CELL'], [('1', 'one\nline')])
        self.assertEqual(records['SHELF_II_CELL'], [('1', 'two')])
        self.assertEqual(m.decode(assets['codex_sol_primary.book']), [('1', 'one\nline')])
        self.assertEqual(m.decode(assets['codex_sol_shelf_ii.book']), [('1', 'two')])
        self.assertIn('catalog_pages::<1>(PROFILE_SHELF_II', code)
        self.assertEqual(len(assets), 4)
    def test_multishelf_rejects_unknown_or_duplicate_routes(self):
        source = '''use super::{Primary, Route, Sub};
const A: Route = Route::new(CELL, '⠁');
Sub { route: A, pages: &["one"] }
Sub { route: A, pages: &["duplicate"] }
'''
        with self.assertRaises(m.Unsupported): m.convert_codex_shelves(source)
        with self.assertRaises(m.Unsupported): m.convert_codex_shelves(source.replace('route: A', 'route: UNKNOWN', 1))
    def test_two_shelves_preserve_route_cells_and_extra_imports(self):
        source = '''use super::{Primary, Raise, Route, Sub};
const A: Route = Route::new(CELL, '⠁');
const B: Route = Route::new(SHELF_CELL, '⠁');
Sub { route: A, pages: &["one"] }
Sub { route: B, pages: &["two"] }
'''
        shelves = {'CELL': ('PRIMARY', 'first.book'), 'SHELF_CELL': ('SHELF', 'second.book')}
        code, assets, records = m.convert_shelves(source, shelves, 'BOOK_')
        self.assertEqual(records['CELL'], [('1', 'one')])
        self.assertEqual(records['SHELF_CELL'], [('1', 'two')])
        self.assertEqual(m.decode(assets['first.book']), [('1', 'one')])
        self.assertEqual(m.decode(assets['second.book']), [('1', 'two')])
        self.assertIn('use super::{Primary, Raise, Route, Sub};', code)
        self.assertIn('catalog_pages::<1>(BOOK_SHELF', code)
if __name__=='__main__': unittest.main()
