#!/usr/bin/env python3
"""Pinta en PNG el volcado JSON de una pantalla (`codeloom_e2e::Snapshot`).

Uso: python scripts/visual/render_screen.py <snapshot.json> <salida.png> [--font-size 16]

Sale con 2 si el JSON no contiene una rejilla `cells`.
"""
import argparse
import json
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

FONTS = [
    r"C:\Windows\Fonts\CascadiaMono.ttf",
    r"C:\Windows\Fonts\consola.ttf",
    r"C:\Windows\Fonts\cour.ttf",
]
DEFAULT_FG = (212, 212, 212)
DEFAULT_BG = (30, 30, 30)
ANSI16 = [
    (0, 0, 0), (205, 49, 49), (13, 188, 121), (229, 229, 16),
    (36, 114, 200), (188, 63, 188), (17, 168, 205), (229, 229, 229),
    (102, 102, 102), (241, 76, 76), (35, 209, 139), (245, 245, 67),
    (59, 142, 234), (214, 112, 214), (41, 184, 219), (255, 255, 255),
]


def xterm_color(index: int):
    if index < 16:
        return ANSI16[index]
    if index < 232:
        index -= 16
        steps = (0, 95, 135, 175, 215, 255)
        return (steps[index // 36], steps[(index // 6) % 6], steps[index % 6])
    gray = 8 + (index - 232) * 10
    return (gray, gray, gray)


def color(spec, default):
    """Convierte la serialización de `ColorSnap` (serde) en una tupla RGB."""
    if spec == "Default" or spec is None:
        return default
    if isinstance(spec, dict) and "Idx" in spec:
        return xterm_color(spec["Idx"])
    if isinstance(spec, dict) and "Rgb" in spec:
        return tuple(spec["Rgb"])
    return default


def load_font(size: int):
    for path in FONTS:
        if Path(path).is_file():
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def render(snapshot: dict, font_size: int) -> Image.Image:
    cells = snapshot["cells"]
    font = load_font(font_size)
    ascent, descent = font.getmetrics()
    cell_w = max(1, round(font.getlength("M")))
    cell_h = ascent + descent
    rows, cols = len(cells), len(cells[0]) if cells else 0
    image = Image.new("RGB", (cols * cell_w, rows * cell_h), DEFAULT_BG)
    draw = ImageDraw.Draw(image)
    for r, row in enumerate(cells):
        for c, cell in enumerate(row):
            fg = color(cell["fg"], DEFAULT_FG)
            bg = color(cell["bg"], DEFAULT_BG)
            if cell["inverse"]:
                fg, bg = bg, fg
            x, y = c * cell_w, r * cell_h
            if bg != DEFAULT_BG:
                draw.rectangle([x, y, x + cell_w - 1, y + cell_h - 1], fill=bg)
            ch = cell["ch"]
            if ch.strip():
                draw.text((x, y), ch, font=font, fill=fg)
                if cell["bold"]:
                    draw.text((x + 1, y), ch, font=font, fill=fg)
            if cell["underline"]:
                draw.line([x, y + cell_h - 2, x + cell_w - 1, y + cell_h - 2], fill=fg)
    cursor_row, cursor_col = snapshot.get("cursor", (0, 0))
    x, y = cursor_col * cell_w, cursor_row * cell_h
    draw.rectangle([x, y, x + cell_w - 1, y + cell_h - 1], outline=(255, 255, 255))
    return image


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("snapshot", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--font-size", type=int, default=16)
    args = parser.parse_args()
    try:
        snapshot = json.loads(args.snapshot.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"no se pudo leer {args.snapshot}: {error}", file=sys.stderr)
        return 2
    if not snapshot.get("cells"):
        print("el JSON no contiene `cells`: no es un Snapshot", file=sys.stderr)
        return 2
    render(snapshot, args.font_size).save(args.output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
