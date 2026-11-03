import json, os, shutil, tempfile, unittest
from pathlib import Path
import importlib.util, sys
ROOT=Path(__file__).resolve().parents[2]
SCRIPT=ROOT/'scripts/runtime/publish-book-catalog.py'
spec=importlib.util.spec_from_file_location('publish',SCRIPT)
publish=importlib.util.module_from_spec(spec); spec.loader.exec_module(publish)
sys.modules['publish']=publish
spec2=importlib.util.spec_from_file_location('read',ROOT/'tests/python/book_catalog_reader_model.py')
read=importlib.util.module_from_spec(spec2); spec2.loader.exec_module(read)

class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(); self.root=Path(self.tmp.name)
        self.book=self.root/'cockpit/book'; self.book.mkdir(parents=True)
        source=ROOT/'cockpit/book'
        for name in publish.EXPECTED: shutil.copyfile(source/name,self.book/name)
        publish.publish(self.root)
    def tearDown(self): self.tmp.cleanup()
    def test_unpublished_edits_fail_then_publish_atomically(self):
        old=read.load(self.root); name='ch_edits.book'; p=self.book/name
        p.write_bytes(p.read_bytes().replace(b'opaque handles',b'opaque handles revised',1))
        with self.assertRaisesRegex(ValueError,'unpublished'): read.load(self.root)
        publish.publish(self.root); new=read.load(self.root)
        self.assertNotEqual(old[name],new[name]); self.assertIn(b'opaque handles',old[name])
    def test_manifest_change_while_reading_is_rejected(self):
        changed=False
        def swap(i,name):
            nonlocal changed
            if not changed:
                changed=True
                m=self.book/'catalog-manifest.json'; m.write_bytes(m.read_bytes()+b' ')
        with self.assertRaisesRegex(ValueError,'manifest changed'): read.load(self.root,swap)
    def test_invalid_publish_keeps_previous_manifest(self):
        p=self.book/'catalog-manifest.json'; before=p.read_bytes()
        source=self.book/'ch_edits.book'; source.write_bytes(source.read_bytes().replace(b'@@book-page 1@@',b'@@book-page 2@@',1))
        with self.assertRaises(ValueError): publish.publish(self.root)
        self.assertEqual(p.read_bytes(),before)
    def test_missing_or_extra_manifest_entries_and_symlink_fail_closed(self):
        p=self.book/'catalog-manifest.json'; data=json.loads(p.read_bytes()); data['chapters'].pop('ch_edits.book'); p.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError,'manifest'): read.load(self.root)
        publish.publish(self.root); chapter=self.book/'ch_edits.book'; chapter.unlink(); chapter.symlink_to(self.book/'sh_shell.book')
        with self.assertRaises(OSError): read.load(self.root)
    def test_duplicate_manifest_keys_are_rejected(self):
        p=self.book/'catalog-manifest.json'; body=p.read_text()
        body=body.replace('{"chapters":', '{"schema":"fake","chapters":',1)
        p.write_text(body)
        with self.assertRaisesRegex(ValueError,'duplicate'): read.load(self.root)

    def test_installed_bundle_publisher_requires_no_checkout(self):
        import subprocess, sys
        bundle=self.root/'isolated-install'
        scripts=bundle/'scripts/runtime'; scripts.mkdir(parents=True)
        dest=bundle/'cockpit/book'; dest.mkdir(parents=True)
        shutil.copyfile(SCRIPT,scripts/'publish-book-catalog.py')
        for name in publish.EXPECTED: shutil.copyfile(self.book/name,dest/name)
        result=subprocess.run([sys.executable,str(scripts/'publish-book-catalog.py')],cwd=self.root,text=True,capture_output=True)
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(len(json.loads((dest/'catalog-manifest.json').read_text())['chapters']),len(publish.EXPECTED))
        self.assertEqual(len(read.load(bundle)),len(publish.EXPECTED))

    def test_publisher_detects_source_change_during_read(self):
        original=publish.read_bounded; n=0
        def changed(path):
            nonlocal n
            n+=1
            data=original(path)
            if n>len(publish.EXPECTED) and path.name=='ch_edits.book': return data+b'x'
            return data
        publish.read_bounded=changed
        try:
            with self.assertRaisesRegex(ValueError,'changed during publication'): publish.publish(self.root)
        finally: publish.read_bounded=original
if __name__=='__main__': unittest.main()
