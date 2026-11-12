# Puts the screenshots of the web UI (left) and the egui UI (right) side by side.
# Usage: uv run --with pillow compare.py [<screenshots dir>]
import sys
from pathlib import Path

from PIL import Image, ImageDraw

root = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).parents[2] / "screenshots")
out = root / "compare"
out.mkdir(exist_ok=True)
for web in sorted((root / "web").glob("*.png")):
    egui = root / "egui" / web.name
    if not egui.exists():
        continue
    a, b = Image.open(web).convert("RGB"), Image.open(egui).convert("RGB")
    gap = 24
    image = Image.new("RGB", (a.width + gap + b.width, max(a.height, b.height) + 40), "white")
    image.paste(a, (0, 40))
    image.paste(b, (a.width + gap, 40))
    draw = ImageDraw.Draw(image)
    draw.rectangle([a.width, 0, a.width + gap - 1, image.height], fill="#888")
    draw.text((8, 8), f"web (React/Tauri): {web.stem}", fill="black", font_size=24)
    draw.text((a.width + gap + 8, 8), f"egui: {web.stem}", fill="black", font_size=24)
    image.save(out / web.name)
    print("wrote", out / web.name)
