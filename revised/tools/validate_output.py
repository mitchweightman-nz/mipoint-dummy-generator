"""Validate a generated dataset using Python 3.12+ standard library only."""
from __future__ import annotations
import argparse
import csv

from pathlib import Path
import posixpath
import xml.etree.ElementTree as ET
import zipfile
csv.field_size_limit(256_000_000)


def validate(root: Path) -> tuple[int, int]:
    manifest = root / 'Structure Inventory.csv'
    with manifest.open(encoding='utf-8-sig', newline='') as handle:
        reader = csv.DictReader(handle)
        assert len(reader.fieldnames or []) == 15, 'Expected five structure fields and ten metadata fields'
        rows = list(reader)
    files = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file() and p != manifest}
    indexed: set[str] = set()
    total = 0
    for row in rows:
        relative = row['RelativePath']
        assert relative not in indexed, f'Duplicate inventory row: {relative}'
        indexed.add(relative)
        path = root / relative
        assert path.resolve().is_relative_to(root.resolve()), 'Inventory path escapes dataset'
        assert path.name == row['FileName']
        assert path.suffix[1:] == row['Extension']
        assert path.stat().st_size == int(row['SizeBytes']), relative
        assert len(Path(relative).parts) - 1 == int(row['FolderDepth']), relative
        assert all(row.values()), f'Missing metadata: {relative}'
        total += path.stat().st_size
        if path.suffix in {'.docx', '.xlsx', '.pptx'}:
            with zipfile.ZipFile(path) as archive:
                assert archive.testzip() is None, f'ZIP CRC failure: {relative}'
                names = set(archive.namelist())
                for name in names:
                    if name.endswith(('.xml', '.rels')):
                        element = ET.fromstring(archive.read(name))
                        if name.endswith('.rels'):
                            owner = '' if name == '_rels/.rels' else name.rsplit('/_rels/', 1)[0]
                            for rel in element:
                                if rel.get('TargetMode') != 'External':
                                    target = posixpath.normpath(posixpath.join(owner, rel.attrib['Target']))
                                    assert target in names, f'Missing relationship target {target}'
                if path.suffix == '.pptx':
                    slide = ET.fromstring(archive.read('ppt/slides/slide1.xml'))
                    ids = [e.attrib['id'] for e in slide.iter() if e.tag.endswith('}cNvPr')]
                    assert len(ids) == len(set(ids)), 'Duplicate slide shape IDs'
        elif path.suffix == '.csv':
            with path.open(encoding='utf-8', newline='') as handle:
                parsed = csv.reader(handle, strict=True)
                for record in parsed:
                    assert len(record) == 5, f'Inconsistent CSV column count: {relative}'
        elif path.suffix == '.rtf':
            text = path.read_text(encoding='ascii')
            assert text.startswith(r'{\rtf1') and text.endswith('}')
            level = 0
            escaped = False
            for char in text:
                if escaped:
                    escaped = False
                elif char == '\\':
                    escaped = True
                elif char == '{':
                    level += 1
                elif char == '}':
                    level -= 1
                    assert level >= 0, f'Unbalanced RTF: {relative}'
            assert level == 0, f'Unbalanced RTF: {relative}'
        else:
            path.read_text(encoding='utf-8')
    assert indexed == files, f'Inventory/file mismatch: {indexed ^ files}'
    return len(rows), total


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dataset', type=Path)
    args = parser.parse_args()
    count, size = validate(args.dataset)
    print(f'Validated {count} files, {size:,} bytes; inventory, paths, metadata, XML, ZIP CRCs, CSV columns and RTF groups passed.')


if __name__ == '__main__':
    main()

