#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PRODUCT="$ROOT/.github/codex-micro-product.webp"
OUTPUT="$ROOT/.github/social-preview.png"
FONT_REGULAR="/System/Library/Fonts/Supplemental/Arial.ttf"
FONT_BOLD="/System/Library/Fonts/Supplemental/Arial Bold.ttf"

command -v magick >/dev/null
test -f "$PRODUCT"
test -f "$FONT_REGULAR"
test -f "$FONT_BOLD"

magick \
  -size 1280x640 'canvas:#EDF6FF' \
  -fill '#2563EB' -stroke none \
  -draw 'roundrectangle 48,72 56,568 4,4' \
  -fill '#F8FBFF' -stroke '#BFD3E6' -strokewidth 2 \
  -draw 'roundrectangle 770,60 1250,580 34,34' \
  -fill '#0F172A' -stroke none -font "$FONT_BOLD" -pointsize 68 \
  -draw "text 82,164 'WorkLouderCTL'" \
  -fill '#334155' -font "$FONT_REGULAR" -pointsize 24 \
  -draw "text 84,224 'Companion CLI for Work Louder Input + Codex Micro'" \
  -fill '#0F172A' -font "$FONT_BOLD" -pointsize 32 \
  -draw "text 82,328 'Plan · Diff · Apply · Verify · Roll Back'" \
  -fill '#F8FBFF' -stroke '#BFD3E6' -strokewidth 2 \
  -draw 'roundrectangle 82,370 215,420 25,25' \
  -draw 'roundrectangle 235,370 355,420 25,25' \
  -draw 'roundrectangle 375,370 555,420 25,25' \
  -draw 'roundrectangle 575,370 705,420 25,25' \
  -fill '#2563EB' -stroke none -font "$FONT_BOLD" -pointsize 18 \
  -draw "text 111,402 'Profiles'" \
  -draw "text 264,402 'Layers'" \
  -draw "text 400,402 'Smart Actions'" \
  -draw "text 599,402 'Rollback'" \
  -fill '#334155' -font "$FONT_REGULAR" -pointsize 18 \
  -draw "text 84,530 'Open source · Agent friendly · Transaction safe'" \
  \( "$PRODUCT" -resize 455x455 \) \
  -geometry +785+92 -composite \
  -background '#EDF6FF' -alpha remove -alpha off \
  -strip \
  "$OUTPUT"

printf 'generated %s\n' "$OUTPUT"
