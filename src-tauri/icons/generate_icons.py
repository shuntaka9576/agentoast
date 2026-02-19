#!/usr/bin/env python3
"""Icon generation script.

Rasterizes agentoast.svg with cairosvg and generates PNGs, icon.icns,
and tray icons at various sizes.

Dependencies: pip install pillow cairosvg
"""

import io
import os
import shutil
import subprocess
import sys
import tempfile

import cairosvg
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

# Source SVG
SVG_PATH = os.path.join(SCRIPT_DIR, "original", "agentoast.svg")

# Output paths
OUTPUT_FILES = {
    32: os.path.join(SCRIPT_DIR, "32x32.png"),
    128: os.path.join(SCRIPT_DIR, "128x128.png"),
    256: os.path.join(SCRIPT_DIR, "128x128@2x.png"),
}
ICNS_PATH = os.path.join(SCRIPT_DIR, "icon.icns")
TRAY_ICON_PATH = os.path.join(SCRIPT_DIR, "tray-icon.png")
TRAY_ICON_NOTIFICATION_PATH = os.path.join(SCRIPT_DIR, "tray-icon-notification.png")

# Toast icon paths (source SVGs and output PNGs)
TOAST_DIR = os.path.join(SCRIPT_DIR, "toast")
TOAST_ICONS = ["claude-code", "codex", "opencode"]
TOAST_ICON_SIZE = 40  # 20pt @2x Retina

# Toast metadata icons (git-branch, tmux)
TOAST_META_ICONS = ["git-branch", "tmux", "x", "trash"]
TOAST_META_ICON_SIZE = 20  # 10pt @2x Retina

# Sizes and filenames required by iconutil
ICONSET_SIZES = {
    "icon_16x16.png": 16,
    "icon_16x16@2x.png": 32,
    "icon_32x32.png": 32,
    "icon_32x32@2x.png": 64,
    "icon_128x128.png": 128,
    "icon_128x128@2x.png": 256,
    "icon_256x256.png": 256,
    "icon_256x256@2x.png": 512,
    "icon_512x512.png": 512,
    "icon_512x512@2x.png": 1024,
}

# Colors
APP_ICON_BG_COLOR = "#5C3A1E"  # Dark brown (burnt toast)
NOTIFICATION_DOT_COLOR = "#FF9500"

# Tray icon size (@2x Retina)
TRAY_SIZE = 44

# Threshold for white background detection (pixels with all RGB channels >= this are considered white)
WHITE_THRESHOLD = 250


def _rasterize_svg(size: int) -> Image.Image:
    """Rasterize SVG to an RGBA PNG at the given size."""
    png_data = cairosvg.svg2png(
        url=SVG_PATH,
        output_width=size,
        output_height=size,
    )
    return Image.open(io.BytesIO(png_data)).convert("RGBA")


def _make_ghost_svg() -> str:
    """Extract the ghost (body + eyes) paths from the SVG and return as an SVG string."""
    with open(SVG_PATH) as f:
        lines = f.readlines()

    parts = []
    parts.append(lines[0])        # SVG header
    parts.extend(lines[2:5])      # Ghost body + left eye + right eye
    parts.append(lines[5])        # </svg>
    return "".join(parts)


def _rasterize_ghost_svg(size: int) -> Image.Image:
    """Rasterize the ghost-only SVG at the given size."""
    ghost_svg = _make_ghost_svg()
    png_data = cairosvg.svg2png(
        bytestring=ghost_svg.encode("utf-8"),
        output_width=size,
        output_height=size,
    )
    return Image.open(io.BytesIO(png_data)).convert("RGBA")


def _remove_white_background(img: Image.Image) -> Image.Image:
    """Make white background transparent (set alpha to 0 for near-white pixels)."""
    arr = np.array(img)
    r, g, b = arr[:, :, 0], arr[:, :, 1], arr[:, :, 2]
    white_mask = (r >= WHITE_THRESHOLD) & (g >= WHITE_THRESHOLD) & (b >= WHITE_THRESHOLD)
    arr[white_mask, 3] = 0
    return Image.fromarray(arr)


def _crop_and_pad(img: Image.Image, target_size: int, padding_ratio: float = 0.05) -> Image.Image:
    """Crop to content bounding box, square, add padding, and resize."""
    bbox = img.getbbox()
    if not bbox:
        return img.resize((target_size, target_size), Image.LANCZOS)

    cropped = img.crop(bbox)
    w, h = cropped.size
    side = max(w, h)

    # Square (center-aligned)
    square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    square.paste(cropped, ((side - w) // 2, (side - h) // 2))

    # Add padding
    padded_side = int(side * (1.0 + padding_ratio * 2))
    padded = Image.new("RGBA", (padded_side, padded_side), (0, 0, 0, 0))
    padded.paste(square, ((padded_side - side) // 2, (padded_side - side) // 2))

    return padded.resize((target_size, target_size), Image.LANCZOS)


def _colorize_to_white(img: Image.Image) -> Image.Image:
    """Convert opaque pixels to white (preserving alpha)."""
    arr = np.array(img)
    opaque = arr[:, :, 3] > 0
    arr[opaque, 0] = 255  # R
    arr[opaque, 1] = 255  # G
    arr[opaque, 2] = 255  # B
    return Image.fromarray(arr)


# --- Icon rendering ---

def render_app_icon(size: int) -> Image.Image:
    """App icon (toast-colored background + character + rounded corners)."""
    canvas_size = 1024

    # Create rounded-rect background with toast color
    result = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(result)
    draw.rounded_rectangle(
        [0, 0, canvas_size - 1, canvas_size - 1],
        radius=225,
        fill=APP_ICON_BG_COLOR,
    )

    # Rasterize SVG, remove white background, colorize to white
    char_size = int(canvas_size * 0.75)
    img = _rasterize_svg(char_size)
    img = _remove_white_background(img)
    img = _colorize_to_white(img)

    # Center character on canvas
    offset = (canvas_size - char_size) // 2
    result.paste(img, (offset, offset), img)

    if size != canvas_size:
        result = result.resize((size, size), Image.LANCZOS)
    return result


def _make_eyes_svg() -> str:
    """Extract the eye paths from the SVG and return as an SVG string."""
    with open(SVG_PATH) as f:
        lines = f.readlines()

    parts = []
    parts.append(lines[0])        # SVG header
    parts.extend(lines[3:5])      # Left eye + right eye
    parts.append(lines[5])        # </svg>
    return "".join(parts)


def _rasterize_eyes_svg(size: int) -> Image.Image:
    """Rasterize the eyes-only SVG at the given size."""
    eyes_svg = _make_eyes_svg()
    png_data = cairosvg.svg2png(
        bytestring=eyes_svg.encode("utf-8"),
        output_width=size,
        output_height=size,
    )
    return Image.open(io.BytesIO(png_data)).convert("RGBA")


def _make_tray_stencil(size: int) -> Image.Image:
    """Generate a stencil with bread outline + ghost solid fill + eyes cut out.

    - Bread: white outline only
    - Ghost body: white solid fill
    - Eyes: transparent (cut out)
    """
    # Ghost silhouette (flood-filled to close holes)
    ghost = _rasterize_ghost_svg(size)
    ghost_arr = np.array(ghost)
    ghost_opaque = ghost_arr[:, :, 3] > 20

    # Flood-fill from corners to identify outside, then invert to get hole-filled silhouette
    ghost_binary = Image.fromarray(ghost_opaque.astype(np.uint8) * 255, mode="L").convert("RGB")
    marker = (255, 0, 0)
    for xy in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)]:
        ImageDraw.floodfill(ghost_binary, xy, marker, thresh=10)
    filled_arr = np.array(ghost_binary)
    outside = (filled_arr[:, :, 0] > 200) & (filled_arr[:, :, 1] < 100)
    ghost_mask = ~outside  # Not outside = full ghost silhouette

    # Eye cutout (dilated slightly larger)
    eyes = _rasterize_eyes_svg(size)
    eyes_arr = np.array(eyes)
    eyes_alpha = Image.fromarray((eyes_arr[:, :, 3] > 20).astype(np.uint8) * 255)
    for _ in range(8):
        eyes_alpha = eyes_alpha.filter(ImageFilter.MaxFilter(3))
    eyes_mask = np.array(eyes_alpha) > 128

    ghost_body = ghost_mask & ~eyes_mask

    # Bread outline = full image (white background removed) - ghost silhouette
    full = _rasterize_svg(size)
    full = _remove_white_background(full)
    full_arr = np.array(full)
    full_opaque = full_arr[:, :, 3] > 20
    bread_outline = full_opaque & ~ghost_mask

    # Composite: bread outline + ghost solid fill
    final = bread_outline | ghost_body

    result = np.zeros((size, size, 4), dtype=np.uint8)
    result[final, 0] = 255  # White
    result[final, 1] = 255
    result[final, 2] = 255
    result[final, 3] = 255
    return Image.fromarray(result)


def render_tray_icon() -> Image.Image:
    """Tray icon (bread outline + ghost solid fill + eyes cut out, 44x44 @2x).
    For icon_as_template(true): macOS uses alpha channel only.
    """
    img = _make_tray_stencil(1024)
    return _crop_and_pad(img, TRAY_SIZE, padding_ratio=0.0)


def render_tray_notification_icon() -> Image.Image:
    """Notification tray icon (bread outline + ghost solid fill + eyes cut out + dot badge, 44x44 @2x).
    For icon_as_template(false).
    """
    img = _make_tray_stencil(1024)

    # Resize stencil to 44x44 first (same size as normal tray icon)
    img = _crop_and_pad(img, TRAY_SIZE, padding_ratio=0.0)

    # Overlay dot badge at top-right on the 44x44 image
    dot_r = 7
    dot_cx = TRAY_SIZE - dot_r
    dot_cy = dot_r

    draw = ImageDraw.Draw(img)
    draw.ellipse(
        [dot_cx - dot_r, dot_cy - dot_r, dot_cx + dot_r, dot_cy + dot_r],
        fill=NOTIFICATION_DOT_COLOR,
    )

    return img


def render_toast_icon(name: str) -> Image.Image:
    """Render a toast icon from SVG to black-on-transparent PNG at TOAST_ICON_SIZE.

    For agent-specific icons (claude-code, codex, opencode), reads from toast/<name>.svg.
    For 'agentoast', reuses the main agentoast.svg.
    """
    if name == "agentoast":
        svg_path = SVG_PATH
    else:
        svg_path = os.path.join(TOAST_DIR, f"{name}.svg")

    png_data = cairosvg.svg2png(
        url=svg_path,
        output_width=TOAST_ICON_SIZE,
        output_height=TOAST_ICON_SIZE,
    )
    img = Image.open(io.BytesIO(png_data)).convert("RGBA")

    # Remove white background (cairosvg may render with white bg)
    img = _remove_white_background(img)

    # Ensure all opaque pixels are black (for template image usage)
    arr = np.array(img)
    opaque = arr[:, :, 3] > 0
    arr[opaque, 0] = 0  # R
    arr[opaque, 1] = 0  # G
    arr[opaque, 2] = 0  # B
    return Image.fromarray(arr)


def render_toast_meta_icon(name: str) -> Image.Image:
    """Render a toast metadata icon from SVG to black-on-transparent PNG at TOAST_META_ICON_SIZE.

    Reads from toast/<name>.svg. Handles both stroke-based (git-branch) and
    fill-based (tmux) SVGs.
    """
    svg_path = os.path.join(TOAST_DIR, f"{name}.svg")

    png_data = cairosvg.svg2png(
        url=svg_path,
        output_width=TOAST_META_ICON_SIZE,
        output_height=TOAST_META_ICON_SIZE,
    )
    img = Image.open(io.BytesIO(png_data)).convert("RGBA")

    # Remove white background (cairosvg may render with white bg)
    img = _remove_white_background(img)

    # Ensure all opaque pixels are black (for template image usage)
    arr = np.array(img)
    opaque = arr[:, :, 3] > 0
    arr[opaque, 0] = 0  # R
    arr[opaque, 1] = 0  # G
    arr[opaque, 2] = 0  # B
    return Image.fromarray(arr)


def main() -> None:
    if not os.path.exists(SVG_PATH):
        print(f"Error: SVG not found: {SVG_PATH}")
        sys.exit(1)

    # App icon PNGs
    for size, output_path in OUTPUT_FILES.items():
        img = render_app_icon(size)
        img.save(output_path)
        print(f"Generated: {output_path} ({size}x{size})")

    # icon.icns
    iconset_dir = tempfile.mkdtemp(suffix=".iconset")
    try:
        for filename, size in ICONSET_SIZES.items():
            output_path = os.path.join(iconset_dir, filename)
            img = render_app_icon(size)
            img.save(output_path)

        result = subprocess.run(
            ["iconutil", "--convert", "icns", iconset_dir, "--output", ICNS_PATH],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"Error running iconutil: {result.stderr}")
            sys.exit(1)

        print(f"Generated: {ICNS_PATH}")
    finally:
        shutil.rmtree(iconset_dir)

    # Tray icon (normal)
    tray = render_tray_icon()
    tray.save(TRAY_ICON_PATH)
    print(f"Generated: {TRAY_ICON_PATH} ({TRAY_SIZE}x{TRAY_SIZE})")

    # Tray icon (notification)
    tray_notification = render_tray_notification_icon()
    tray_notification.save(TRAY_ICON_NOTIFICATION_PATH)
    print(f"Generated: {TRAY_ICON_NOTIFICATION_PATH} ({TRAY_SIZE}x{TRAY_SIZE})")

    # Toast icons (agent-specific + agentoast)
    all_toast_icons = TOAST_ICONS + ["agentoast"]
    for name in all_toast_icons:
        img = render_toast_icon(name)
        output_path = os.path.join(TOAST_DIR, f"{name}.png")
        img.save(output_path)
        print(f"Generated: {output_path} ({TOAST_ICON_SIZE}x{TOAST_ICON_SIZE})")

    # Toast metadata icons (git-branch, tmux)
    for name in TOAST_META_ICONS:
        img = render_toast_meta_icon(name)
        output_path = os.path.join(TOAST_DIR, f"{name}.png")
        img.save(output_path)
        print(f"Generated: {output_path} ({TOAST_META_ICON_SIZE}x{TOAST_META_ICON_SIZE})")

    print("\nDone!")


if __name__ == "__main__":
    main()
