#!/usr/bin/env python3
import json
import os
import sys

ROWS = 6


def scenes(directory):
    names = set()
    for entry in os.listdir(directory):
        stem, ext = os.path.splitext(entry)
        if ext in (".txt", ".json"):
            names.add(stem)
    return names


def read_text(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return handle.read()
    except FileNotFoundError:
        return None


def validate_sheet(sheet):
    if not isinstance(sheet, dict):
        raise ValueError("sheet must be an object")
    for dimension in ("width", "height"):
        value = sheet.get(dimension)
        if type(value) is not int or value <= 0:
            raise ValueError(f"{dimension} must be a positive integer")
    palette = sheet.get("palette")
    if not isinstance(palette, list) or not all(
        isinstance(entry, list) and len(entry) == 3
        and all(isinstance(value, str) for value in entry)
        for entry in palette
    ):
        raise ValueError("palette must contain three-string styles")
    cells = sheet.get("cells")
    if not isinstance(cells, list):
        raise ValueError("cells must be an array")
    expected = sheet["width"] * sheet["height"]
    if len(cells) != expected:
        raise ValueError(f"cell count {len(cells)} does not match width * height ({expected})")
    for position, cell in enumerate(cells):
        if not isinstance(cell, list) or len(cell) != 2 or not isinstance(cell[0], str):
            raise ValueError(f"cell {position} must be a [symbol, palette index] pair")
        index = cell[1]
        if type(index) is not int or not 0 <= index < len(palette):
            raise ValueError(f"cell {position} has invalid palette index {index!r}")


def read_sheet(path):
    try:
        with open(path, encoding="utf-8") as handle:
            sheet = json.load(handle)
        validate_sheet(sheet)
        return sheet
    except FileNotFoundError:
        return None
    except (OSError, UnicodeError, ValueError) as error:
        raise ValueError(f"{path}: {error}") from error


def styled_cells(sheet):
    width = sheet["width"]
    palette = [tuple(entry) for entry in sheet["palette"]]
    cells = []
    for symbol, index in sheet["cells"]:
        cells.append((symbol, palette[index]))
    return width, cells


def text_diff(a_text, b_text):
    a_rows = a_text.split("\n")
    b_rows = b_text.split("\n")
    out = []
    for row in range(max(len(a_rows), len(b_rows))):
        a_row = a_rows[row] if row < len(a_rows) else "<missing>"
        b_row = b_rows[row] if row < len(b_rows) else "<missing>"
        if a_row != b_row:
            out.append((row, a_row, b_row))
    return out


def style_diff(a_sheet, b_sheet):
    if a_sheet["width"] != b_sheet["width"] or a_sheet["height"] != b_sheet["height"]:
        return None
    width, a_cells = styled_cells(a_sheet)
    _, b_cells = styled_cells(b_sheet)
    out = []
    for position, (a_cell, b_cell) in enumerate(zip(a_cells, b_cells)):
        if a_cell[1] != b_cell[1]:
            out.append((position % width, position // width, a_cell, b_cell))
    return out


def main(argv):
    if len(argv) != 3:
        sys.stderr.write("usage: compare.py DIR_A DIR_B\n")
        return 2
    dir_a, dir_b = argv[1], argv[2]
    try:
        names_a, names_b = scenes(dir_a), scenes(dir_b)
        if not names_a and not names_b:
            raise ValueError(f"no scenes found in {dir_a} or {dir_b}")
    except (OSError, ValueError) as error:
        print(f"invalid capture: {error}", file=sys.stderr)
        return 2
    differing = 0
    for name in sorted(names_a | names_b):
        try:
            a_text = read_text(os.path.join(dir_a, name + ".txt"))
            b_text = read_text(os.path.join(dir_b, name + ".txt"))
            a_sheet = read_sheet(os.path.join(dir_a, name + ".json"))
            b_sheet = read_sheet(os.path.join(dir_b, name + ".json"))
        except (OSError, UnicodeError, ValueError) as error:
            print(f"invalid capture for {name}: {error}", file=sys.stderr)
            return 2
        if a_text is None or b_text is None or a_sheet is None or b_sheet is None:
            differing += 1
            print(f"{name}: present in only one directory or missing a file")
            continue
        rows = text_diff(a_text, b_text)
        styles = style_diff(a_sheet, b_sheet)
        symbols_differ = [
            position
            for position, (a_cell, b_cell) in enumerate(
                zip(a_sheet["cells"], b_sheet["cells"])
            )
            if a_cell[0] != b_cell[0]
        ]
        if not rows and not symbols_differ and styles == []:
            continue
        differing += 1
        if styles is None:
            style_note = "styles: sizes differ"
        elif styles:
            style_note = f"styles differ in {len(styles)} cells"
        else:
            style_note = "styles identical"
        print(f"{name}: text rows differ {len(rows)}, {style_note}")
        for row, a_row, b_row in rows[:ROWS]:
            print(f"  row {row}")
            print(f"    A: {a_row}")
            print(f"    B: {b_row}")
        if styles:
            for x, y, a_cell, b_cell in styles[:ROWS]:
                print(f"  style at ({x},{y}) {a_cell[0]!r}")
                print(f"    A: fg={a_cell[1][0]} bg={a_cell[1][1]} mod={a_cell[1][2]}")
                print(f"    B: fg={b_cell[1][0]} bg={b_cell[1][1]} mod={b_cell[1][2]}")
    total = len(names_a | names_b)
    if differing:
        print(f"{differing} of {total} scenes differ")
        return 1
    print(f"all {total} scenes identical")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
