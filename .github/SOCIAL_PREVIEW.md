# Social preview source

The GitHub social preview uses the real Codex Micro product render in
`codex-micro-product.webp` rather than an abstract keypad illustration.

The current `social-preview.png` was composed from that transparent product
render and the WorkLouderCTL title card at GitHub's recommended 1280×640 aspect
ratio. The layout uses a solid `#EDF6FF` background and flat-color elements;
there are no designed gradients, glows, or drop shadows. Keep the product
geometry, key layout, controls, and perspective faithful to the committed render
when revising the preview.

Regenerate it on macOS with ImageMagick:

```console
scripts/generate_social_preview.sh
```

## Required output checks

- PNG format;
- exactly 1280×640 pixels;
- less than 1 MB;
- no transparency;
- background pixels outside the artwork are exactly `#EDF6FF`;
- no gradients, glows, or drop shadows in designed elements;
- readable title and subtitle at link-preview size;
- the complete product remains inside the right-hand safe area.
