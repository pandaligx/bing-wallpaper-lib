"""Measure text edges in the sidebar before and after opening the native viewer."""

import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def edge_rms(image, bounds):
    region = image.convert("L").crop(bounds)
    w, h = region.size
    left = region.crop((0, 0, w - 1, h))
    right = region.crop((1, 0, w, h))
    return ImageStat.Stat(ImageChops.difference(left, right)).rms[0]


def check(directory, theme):
    with Image.open(directory / f"{theme}-library.png") as library, Image.open(
        directory / f"{theme}-viewer.png"
    ) as viewer:
        assert library.size == viewer.size
        w, h = library.size
        # The Settings footer lies below the displayed photo and to the left of the controls.
        footer = (6, h - 36, 138, h - 5)
        before, after = edge_rms(library, footer), edge_rms(viewer, footer)
        photo = edge_rms(viewer, (w // 3, h // 3, 2 * w // 3, 2 * h // 3))
        print(f"{theme}: sidebar edges {before:.2f} -> {after:.2f}, photo edges {photo:.2f}")
        if before < 5 or after > before * 0.35 or photo < 5:
            raise ValueError("The sidebar must blur while the foreground photo stays sharp")


if __name__ == "__main__":
    for scheme in ("light", "dark"):
        check(Path(sys.argv[1]), scheme)
