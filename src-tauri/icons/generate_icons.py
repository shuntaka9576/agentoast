#!/usr/bin/env python3
"""アイコン生成スクリプト

agentoast.svg を cairosvg でラスタライズし、
各サイズの PNG と icon.icns、トレイアイコンを生成する。

依存: pip install pillow cairosvg
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

# ソース SVG
SVG_PATH = os.path.join(SCRIPT_DIR, "original", "agentoast.svg")

# 生成先
OUTPUT_FILES = {
    32: os.path.join(SCRIPT_DIR, "32x32.png"),
    128: os.path.join(SCRIPT_DIR, "128x128.png"),
    256: os.path.join(SCRIPT_DIR, "128x128@2x.png"),
}
ICNS_PATH = os.path.join(SCRIPT_DIR, "icon.icns")
TRAY_ICON_PATH = os.path.join(SCRIPT_DIR, "tray-icon.png")
TRAY_ICON_NOTIFICATION_PATH = os.path.join(SCRIPT_DIR, "tray-icon-notification.png")

# iconutil に必要なサイズとファイル名
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

# 色定義
APP_ICON_BG_COLOR = "#5C3A1E"  # ダークブラウン (焦げトースト)
NOTIFICATION_DOT_COLOR = "#5181B8"

# トレイアイコンサイズ (@2x Retina)
TRAY_SIZE = 44

# 白背景と判定する閾値 (RGB 各チャンネルがこの値以上なら白とみなす)
WHITE_THRESHOLD = 250


def _rasterize_svg(size: int) -> Image.Image:
    """SVG を指定サイズの RGBA PNG にラスタライズ"""
    png_data = cairosvg.svg2png(
        url=SVG_PATH,
        output_width=size,
        output_height=size,
    )
    return Image.open(io.BytesIO(png_data)).convert("RGBA")


def _make_ghost_svg() -> str:
    """SVG からお化け部分（本体 + 目）のみを抽出した SVG 文字列を返す。"""
    with open(SVG_PATH) as f:
        lines = f.readlines()

    parts = []
    parts.append(lines[0])        # SVG header
    parts.extend(lines[2:5])      # ゴースト本体 + 左目 + 右目
    parts.append(lines[5])        # </svg>
    return "".join(parts)


def _rasterize_ghost_svg(size: int) -> Image.Image:
    """お化け部分のみの SVG を指定サイズにラスタライズ"""
    ghost_svg = _make_ghost_svg()
    png_data = cairosvg.svg2png(
        bytestring=ghost_svg.encode("utf-8"),
        output_width=size,
        output_height=size,
    )
    return Image.open(io.BytesIO(png_data)).convert("RGBA")


def _remove_white_background(img: Image.Image) -> Image.Image:
    """白背景を透明化 (白に近いピクセルの alpha を 0 にする)"""
    arr = np.array(img)
    r, g, b = arr[:, :, 0], arr[:, :, 1], arr[:, :, 2]
    white_mask = (r >= WHITE_THRESHOLD) & (g >= WHITE_THRESHOLD) & (b >= WHITE_THRESHOLD)
    arr[white_mask, 3] = 0
    return Image.fromarray(arr)


def _crop_and_pad(img: Image.Image, target_size: int, padding_ratio: float = 0.05) -> Image.Image:
    """コンテンツ領域にクロップ → 正方形化 → パディング → リサイズ"""
    bbox = img.getbbox()
    if not bbox:
        return img.resize((target_size, target_size), Image.LANCZOS)

    cropped = img.crop(bbox)
    w, h = cropped.size
    side = max(w, h)

    # 正方形化 (中央配置)
    square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    square.paste(cropped, ((side - w) // 2, (side - h) // 2))

    # パディング追加
    padded_side = int(side * (1.0 + padding_ratio * 2))
    padded = Image.new("RGBA", (padded_side, padded_side), (0, 0, 0, 0))
    padded.paste(square, ((padded_side - side) // 2, (padded_side - side) // 2))

    return padded.resize((target_size, target_size), Image.LANCZOS)


def _colorize_to_white(img: Image.Image) -> Image.Image:
    """不透明ピクセルを白に変換 (alpha はそのまま)"""
    arr = np.array(img)
    opaque = arr[:, :, 3] > 0
    arr[opaque, 0] = 255  # R
    arr[opaque, 1] = 255  # G
    arr[opaque, 2] = 255  # B
    return Image.fromarray(arr)


# --- アイコン描画 ---

def render_app_icon(size: int) -> Image.Image:
    """アプリアイコン (トースト色背景 + キャラクター + 角丸マスク)"""
    canvas_size = 1024

    # トースト色の角丸背景を作成
    result = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(result)
    draw.rounded_rectangle(
        [0, 0, canvas_size - 1, canvas_size - 1],
        radius=225,
        fill=APP_ICON_BG_COLOR,
    )

    # SVG をラスタライズして白背景を除去 → 白に色変換
    char_size = int(canvas_size * 0.75)
    img = _rasterize_svg(char_size)
    img = _remove_white_background(img)
    img = _colorize_to_white(img)

    # キャラクターを中央に配置
    offset = (canvas_size - char_size) // 2
    result.paste(img, (offset, offset), img)

    if size != canvas_size:
        result = result.resize((size, size), Image.LANCZOS)
    return result


def _make_eyes_svg() -> str:
    """SVG から目のパスのみを抽出した SVG 文字列を返す。"""
    with open(SVG_PATH) as f:
        lines = f.readlines()

    parts = []
    parts.append(lines[0])        # SVG header
    parts.extend(lines[3:5])      # 左目 + 右目
    parts.append(lines[5])        # </svg>
    return "".join(parts)


def _rasterize_eyes_svg(size: int) -> Image.Image:
    """目のみの SVG を指定サイズにラスタライズ"""
    eyes_svg = _make_eyes_svg()
    png_data = cairosvg.svg2png(
        bytestring=eyes_svg.encode("utf-8"),
        output_width=size,
        output_height=size,
    )
    return Image.open(io.BytesIO(png_data)).convert("RGBA")


def _make_tray_stencil(size: int) -> Image.Image:
    """パン線画 + お化け白塗りつぶし + 目くり抜きのステンシルを生成。

    - パン: アウトライン(線)のみ白で表示
    - お化けボディ: 白で塗りつぶし
    - 目: 透明(くり抜き)
    """
    # お化けシルエット (フラッドフィルで穴を埋めた塗りつぶし)
    ghost = _rasterize_ghost_svg(size)
    ghost_arr = np.array(ghost)
    ghost_opaque = ghost_arr[:, :, 3] > 20

    # 四隅からフラッドフィルで外側を特定し、穴を埋めたシルエットを生成
    ghost_binary = Image.fromarray(ghost_opaque.astype(np.uint8) * 255, mode="L").convert("RGB")
    marker = (255, 0, 0)
    for xy in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)]:
        ImageDraw.floodfill(ghost_binary, xy, marker, thresh=10)
    filled_arr = np.array(ghost_binary)
    outside = (filled_arr[:, :, 0] > 200) & (filled_arr[:, :, 1] < 100)
    ghost_mask = ~outside  # 外側でない = お化けシルエット全体

    # 目くり抜き (膨張で少し大きく)
    eyes = _rasterize_eyes_svg(size)
    eyes_arr = np.array(eyes)
    eyes_alpha = Image.fromarray((eyes_arr[:, :, 3] > 20).astype(np.uint8) * 255)
    for _ in range(8):
        eyes_alpha = eyes_alpha.filter(ImageFilter.MaxFilter(3))
    eyes_mask = np.array(eyes_alpha) > 128

    ghost_body = ghost_mask & ~eyes_mask

    # パン線画 = 全体(白背景除去後) - お化け全体
    full = _rasterize_svg(size)
    full = _remove_white_background(full)
    full_arr = np.array(full)
    full_opaque = full_arr[:, :, 3] > 20
    bread_outline = full_opaque & ~ghost_mask

    # 合成: パン線画 + お化け塗りつぶし
    final = bread_outline | ghost_body

    result = np.zeros((size, size, 4), dtype=np.uint8)
    result[final, 0] = 255  # 白
    result[final, 1] = 255
    result[final, 2] = 255
    result[final, 3] = 255
    return Image.fromarray(result)


def render_tray_icon() -> Image.Image:
    """トレイアイコン (パン線画 + お化け白塗り + 目くり抜き、44x44 @2x)
    icon_as_template(true) 用: macOS が alpha チャンネルのみ使用
    """
    img = _make_tray_stencil(1024)
    return _crop_and_pad(img, TRAY_SIZE, padding_ratio=0.0)


def render_tray_notification_icon() -> Image.Image:
    """通知ありトレイアイコン (パン線画 + お化け白塗り + 目くり抜き + 青ドット、44x44 @2x)
    icon_as_template(false) 用
    """
    img = _make_tray_stencil(1024)

    # 先にステンシルを44x44にリサイズ (通常アイコンと同じサイズにする)
    img = _crop_and_pad(img, TRAY_SIZE, padding_ratio=0.0)

    # 44x44のまま右上に丸バッジを重ねる
    dot_r = 7
    dot_cx = TRAY_SIZE - dot_r
    dot_cy = dot_r

    draw = ImageDraw.Draw(img)
    draw.ellipse(
        [dot_cx - dot_r, dot_cy - dot_r, dot_cx + dot_r, dot_cy + dot_r],
        fill=NOTIFICATION_DOT_COLOR,
    )

    return img


def main() -> None:
    if not os.path.exists(SVG_PATH):
        print(f"Error: SVG not found: {SVG_PATH}")
        sys.exit(1)

    # アプリアイコン PNG
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

    # トレイアイコン (通常)
    tray = render_tray_icon()
    tray.save(TRAY_ICON_PATH)
    print(f"Generated: {TRAY_ICON_PATH} ({TRAY_SIZE}x{TRAY_SIZE})")

    # トレイアイコン (通知あり)
    tray_notification = render_tray_notification_icon()
    tray_notification.save(TRAY_ICON_NOTIFICATION_PATH)
    print(f"Generated: {TRAY_ICON_NOTIFICATION_PATH} ({TRAY_SIZE}x{TRAY_SIZE})")

    print("\nDone!")


if __name__ == "__main__":
    main()
