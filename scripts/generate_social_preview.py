#!/usr/bin/env python3
"""Generate the 1280x640 GitHub social preview for WorkLouderCTL."""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".github" / "social-preview.png"
WIDTH, HEIGHT = 1280, 640

FONT_REGULAR = "/System/Library/Fonts/Supplemental/Arial.ttf"
FONT_BOLD = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"


def font(size: int, bold: bool = False) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(FONT_BOLD if bold else FONT_REGULAR, size)


def rounded(draw: ImageDraw.ImageDraw, box, radius, fill, outline=None, width=1):
    draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


def main() -> None:
    image = Image.new("RGB", (WIDTH, HEIGHT), "#090B10")
    pixels = image.load()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            t = (x / WIDTH + y / HEIGHT) / 2
            pixels[x, y] = (
                int(9 + (17 - 9) * t),
                int(11 + (24 - 11) * t),
                int(16 + (39 - 16) * t),
            )

    draw = ImageDraw.Draw(image)

    # Accent rule.
    for y in range(72, 568):
        t = (y - 72) / (568 - 72)
        if t < 0.55:
            u = t / 0.55
            color = (255, int(107 + (158 - 107) * u), int(53 - 42 * u))
        else:
            u = (t - 0.55) / 0.45
            color = (int(245 - 211 * u), int(158 + 39 * u), int(11 + 69 * u))
        draw.rounded_rectangle((68, y, 77, y + 2), radius=4, fill=color)

    # Soft colored light behind the device illustration.
    glow = Image.new("RGBA", image.size, (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.ellipse((880, 55, 1290, 465), fill=(48, 79, 254, 38))
    gd.ellipse((830, 310, 1220, 700), fill=(255, 107, 53, 28))
    glow = glow.filter(ImageFilter.GaussianBlur(55))
    image = Image.alpha_composite(image.convert("RGBA"), glow).convert("RGB")
    draw = ImageDraw.Draw(image)

    # Copy.
    draw.text((115, 115), "WorkLouderCTL", font=font(67, True), fill="#F9FAFB")
    draw.text(
        (119, 205),
        "Companion CLI for Work Louder Input + Codex Micro",
        font=font(23),
        fill="#A7B0C0",
    )
    draw.text(
        (119, 292),
        "Plan · Diff · Apply · Verify · Roll Back",
        font=font(27, True),
        fill="#E5E7EB",
    )

    labels = [("Profiles", 118), ("Layers", 256), ("Smart Actions", 382), ("Rollback", 566)]
    widths = {"Profiles": 122, "Layers": 110, "Smart Actions": 168, "Rollback": 124}
    for label, x in labels:
        w = widths[label]
        rounded(draw, (x, 355, x + w, 401), 23, "#1F2937", "#3B4758", 2)
        bbox = draw.textbbox((0, 0), label, font=font(17, True))
        tx = x + (w - (bbox[2] - bbox[0])) / 2
        draw.text((tx, 368), label, font=font(17, True), fill="#D1D5DB")

    draw.text(
        (119, 500),
        "Open source · Agent friendly · Transaction safe",
        font=font(18),
        fill="#70798A",
    )

    # Abstract Codex Micro control surface.
    rounded(draw, (872, 135, 1210, 470), 52, "#0B0F16", "#3A4658", 3)
    rounded(draw, (892, 155, 1190, 450), 39, "#121923", "#222C3A", 2)

    key_glow = Image.new("RGBA", image.size, (0, 0, 0, 0))
    kg = ImageDraw.Draw(key_glow)
    key_colors = ["#304FFE", "#FF6D00", "#00C853"]
    for i, color in enumerate(key_colors):
        x = 917 + i * 81
        kg.rounded_rectangle((x - 10, 185, x + 76, 270), radius=25, fill=color + "66")
    key_glow = key_glow.filter(ImageFilter.GaussianBlur(14))
    image = Image.alpha_composite(image.convert("RGBA"), key_glow).convert("RGB")
    draw = ImageDraw.Draw(image)

    for i, color in enumerate(key_colors):
        x = 917 + i * 81
        rounded(draw, (x, 195, x + 66, 261), 16, color, None)
    for row, y in enumerate((276, 357)):
        height = 64 if row == 0 else 55
        for i in range(3):
            x = 917 + i * 81
            rounded(draw, (x, y, x + 66, y + height), 16, "#202938", "#3A4658", 2)

    draw.ellipse((1150, 192, 1202, 244), fill="#202938", outline="#52647E", width=3)
    draw.ellipse((1155, 290, 1197, 332), fill="#151D29", outline="#52647E", width=3)
    draw.line((1176, 274, 1176, 348), fill="#52647E", width=4)
    draw.line((1139, 311, 1213, 311), fill="#52647E", width=4)

    footer = "github.com/MarlinDiary/worklouder-input-cli"
    bbox = draw.textbbox((0, 0), footer, font=font(14))
    draw.text((WIDTH - 45 - (bbox[2] - bbox[0]), 591), footer, font=font(14), fill="#566071")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    image.save(OUT, format="PNG", optimize=True)


if __name__ == "__main__":
    main()
