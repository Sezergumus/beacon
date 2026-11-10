"""Turns oversized animated item icons in public/items/ into small single-frame PNGs.

Some wiki icons are GIFs with hundreds of frames (enchantment glint loops) and weigh
hundreds of KB. Run this after `npm run items:generate`:  python scripts/shrink-item-icons.py
Needs Pillow (`pip install pillow`).
"""
from pathlib import Path

from PIL import Image, ImageChops, ImageSequence

ICON_DIR = Path(__file__).resolve().parent.parent / "public" / "items"
SIZE_LIMIT = 20_000  # bytes; smaller animations (clock, compass, ...) keep playing
ICON_SIZE = 32

# For glint icons, keep the frame that differs most from the plain item (= glint at its brightest).
PLAIN_VERSION = {"enchanted_golden_apple": "golden_apple", "debug_stick": "stick"}


def is_gif(path: Path) -> bool:
    with path.open("rb") as f:
        return f.read(3) == b"GIF"


def difference(a: Image.Image, b: Image.Image) -> int:
    return sum(ImageChops.difference(a, b).convert("L").getdata())


def pick_frame(path: Path) -> Image.Image:
    with Image.open(path) as im:
        frames = [f.convert("RGBA") for f in ImageSequence.Iterator(im)]
    plain_name = PLAIN_VERSION.get(path.stem)
    plain_path = ICON_DIR / f"{plain_name}.png" if plain_name else None
    if plain_path and plain_path.exists() and not is_gif(plain_path):
        plain = Image.open(plain_path).convert("RGBA").resize(frames[0].size, Image.NEAREST)
        return max(frames, key=lambda f: difference(f, plain))
    return frames[0]


for path in sorted(ICON_DIR.glob("*.png")):
    if not is_gif(path) or path.stat().st_size <= SIZE_LIMIT:
        continue
    before = path.stat().st_size
    frame = pick_frame(path)
    if frame.width > ICON_SIZE:
        frame = frame.resize((ICON_SIZE, ICON_SIZE), Image.LANCZOS)
    frame.save(path, format="PNG", optimize=True)
    print(f"{path.name}: {before:,} -> {path.stat().st_size:,} bytes")
