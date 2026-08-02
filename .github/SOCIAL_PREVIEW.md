# Social preview source

The GitHub social preview uses the real Codex Micro product render in
`codex-micro-product.webp` rather than an abstract keypad illustration.

The current `social-preview.png` was composed from that transparent product
render and the WorkLouderCTL title card, then normalized to GitHub's recommended
1280×640 aspect ratio. Keep the product geometry, key layout, controls, and
perspective faithful to the committed render when revising the preview.

## Required output checks

- PNG format;
- exactly 1280×640 pixels;
- less than 1 MB;
- no transparency;
- readable title and subtitle at link-preview size;
- the complete product remains inside the right-hand safe area.
