#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ANSI = re.compile(r"\x1b\[[0-9;]*m")
WIDTH, HEIGHT = 1100, 650
FRAME_MS = 250
BG = "#07111f"
PANEL = "#0e1625"
TEXT = "#dce8f7"
MUTED = "#7f94b2"
ACCENT = "#58d59b"


def font(size: int):
    for candidate in ("C:/Windows/Fonts/consola.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"):
        if Path(candidate).exists():
            return ImageFont.truetype(candidate, size)
    return ImageFont.load_default()


def render(cast_path: Path, output_path: Path) -> None:
    lines = cast_path.read_text(encoding="utf-8").splitlines()
    header = json.loads(lines[0])
    events = [json.loads(line) for line in lines[1:]]
    duration = float(header.get("duration", events[-1][0]))
    body_font = font(18)
    title_font = font(16)
    frames = []
    event_index = 0
    screen = ""

    for step in range(int(duration * 1000 / FRAME_MS) + 1):
        now = step * FRAME_MS / 1000
        while event_index < len(events) and events[event_index][0] <= now:
            screen += ANSI.sub("", events[event_index][2])
            event_index += 1

        visible = screen.replace("\r", "").splitlines()[-24:]
        image = Image.new("RGB", (WIDTH, HEIGHT), BG)
        draw = ImageDraw.Draw(image)
        draw.rounded_rectangle((18, 18, WIDTH - 18, HEIGHT - 18), radius=8, fill=PANEL, outline="#26364d", width=2)
        draw.ellipse((38, 38, 52, 52), fill="#ff7d94")
        draw.ellipse((62, 38, 76, 52), fill="#ffd56b")
        draw.ellipse((86, 38, 100, 52), fill=ACCENT)
        draw.text((126, 35), header.get("title", "Terminal"), font=title_font, fill=MUTED)
        y = 78
        for line in visible:
            color = ACCENT if line.startswith("PS>") or "passed" in line or "No active" in line else TEXT
            draw.text((42, y), line[:102], font=body_font, fill=color)
            y += 23
        frames.append(image.quantize(colors=96))

    output_path.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(output_path, save_all=True, append_images=frames[1:], duration=FRAME_MS, loop=0, optimize=True)


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    source = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "docs/assets/terminal-demo.cast"
    output = Path(sys.argv[2]) if len(sys.argv) > 2 else root / "docs/assets/terminal-demo.gif"
    render(source, output)
