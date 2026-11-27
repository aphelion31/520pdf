#!/usr/bin/env python3
"""High-fidelity PDF-to-DOCX converter with per-page hybrid OCR and layout reconstruction."""

from __future__ import annotations

import argparse
import copy
import math
import os
import pathlib
import re
import sys
import tempfile
import zipfile

import cv2
import numpy as np
import pymupdf
from lxml import etree
from pdf2docx import Converter

W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
R_NS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
WP_NS = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
A_NS = "http://schemas.openxmlformats.org/drawingml/2006/main"
W_TAG = f"{{{W_NS}}}"
WP_TAG = f"{{{WP_NS}}}"
A_TAG = f"{{{A_NS}}}"

SUBSET_PREFIX_RE = re.compile(r"^[A-Z]{6}\+")
STYLE_SUFFIX_RE = re.compile(
    r"(?:[-_,](?:BoldItalic|BoldOblique|Bold|Italic|Oblique|Regular|Medium|Light|Semibold|Demi|Black|PSMT|MT))+$",
    re.IGNORECASE,
)

FONT_ALIAS_MAP = {
    "helvetica": "Arial",
    "helveticaneue": "Arial",
    "helv": "Arial",
    "arial": "Arial",
    "arialmt": "Arial",
    "times": "Times New Roman",
    "timesroman": "Times New Roman",
    "timesnewroman": "Times New Roman",
    "timesnewromanpsmt": "Times New Roman",
    "courier": "Courier New",
    "couriernew": "Courier New",
    "courierstd": "Courier New",
    "carlito": "Calibri",
    "caladea": "Cambria",
    "liberationsans": "Arial",
    "liberationserif": "Times New Roman",
    "liberationmono": "Courier New",
    "nimbussans": "Arial",
    "nimbussansl": "Arial",
    "nimbusroman": "Times New Roman",
}


def find_tessdata_and_lang(requested_lang: str) -> tuple[str | None, str]:
    repo_root = pathlib.Path(__file__).resolve().parent.parent
    candidates = [
        os.environ.get("TESSDATA_PREFIX", "").strip(),
        str(repo_root / ".tessdata"),
        "/opt/homebrew/share/tessdata",
        "/usr/local/share/tessdata",
        "/usr/share/tesseract-ocr/5/tessdata",
        "/usr/share/tesseract-ocr/4.00/tessdata",
        "/usr/share/tessdata",
    ]
    seen_dirs = [pathlib.Path(c) for c in candidates if c and pathlib.Path(c).is_dir()]
    if not seen_dirs and hasattr(pymupdf, "get_tessdata"):
        try:
            td = pymupdf.get_tessdata()
            if td and pathlib.Path(td).is_dir():
                seen_dirs.append(pathlib.Path(td))
        except Exception:
            pass

    req_parts = [p.strip() for p in requested_lang.split("+") if p.strip()]
    if not req_parts:
        req_parts = ["eng"]

    for d in seen_dirs:
        matched = [p for p in req_parts if (d / f"{p}.traineddata").is_file()]
        if matched:
            return str(d), "+".join(matched)
    for d in seen_dirs:
        if (d / "eng.traineddata").is_file():
            return str(d), "eng"
    return None, "eng"


def find_unicode_ttf(bold: bool) -> str | None:
    paths = (
        [
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "/Library/Fonts/Arial Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        ]
        if bold
        else [
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/Library/Fonts/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ]
    )
    for p in paths:
        if pathlib.Path(p).is_file():
            return p
    return None


def classify_page(page: pymupdf.Page, force_ocr: bool) -> str:
    if force_ocr:
        return "ocr_needed"
    vis_chars = 0
    hid_chars = 0
    try:
        traces = page.get_texttrace()
    except Exception:
        traces = []

    if traces:
        for span in traces:
            chars = sum(1 for ch in span.get("chars", ()) if ch[0] > 32)
            if span.get("type") == 3:
                hid_chars += chars
            else:
                vis_chars += chars
    else:
        vis_chars = len("".join(page.get_text("text").split()))

    if vis_chars >= 20:
        return "digital"
    if hid_chars >= 20:
        return "hidden_ocr"
    return "ocr_needed"


def estimate_skew_angle(gray: np.ndarray) -> float:
    _, bw = cv2.threshold(gray, 0, 255, cv2.THRESH_BINARY_INV + cv2.THRESH_OTSU)
    ker = cv2.getStructuringElement(cv2.MORPH_RECT, (25, 3))
    merged = cv2.morphologyEx(bw, cv2.MORPH_CLOSE, ker)
    contours, _ = cv2.findContours(merged, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    angles: list[float] = []
    for cnt in sorted(contours, key=cv2.contourArea, reverse=True)[:45]:
        rect = cv2.minAreaRect(cnt)
        w, h = rect[1]
        if max(w, h) < 40 or min(w, h) < 3:
            continue
        ang = rect[2]
        if w < h:
            ang += 90.0
        if ang > 45.0:
            ang -= 90.0
        elif ang <= -45.0:
            ang += 90.0
        if -12.0 <= ang <= 12.0:
            angles.append(ang)
    if len(angles) < 3:
        return 0.0
    med = float(np.median(angles))
    return med if 0.35 <= abs(med) <= 8.0 else 0.0


def rotate_rgb(img: np.ndarray, angle_deg: float) -> np.ndarray:
    h, w = img.shape[:2]
    mat = cv2.getRotationMatrix2D((w * 0.5, h * 0.5), angle_deg, 1.0)
    return cv2.warpAffine(
        img,
        mat,
        (w, h),
        flags=cv2.INTER_CUBIC,
        borderMode=cv2.BORDER_CONSTANT,
        borderValue=(255, 255, 255),
    )


def detect_banners_and_lines(
    img: np.ndarray,
    pw: float,
    ph: float,
) -> tuple[
    list[tuple[float, float, float, float, tuple[float, float, float]]],
    list[tuple[float, float, float, float, tuple[float, float, float]]],
    list[tuple[float, float, float, float, tuple[float, float, float]]],
    np.ndarray,
    np.ndarray,
    np.ndarray,
]:
    ih, iw = img.shape[:2]
    sx, sy = pw / iw, ph / ih
    gray = cv2.cvtColor(img, cv2.COLOR_RGB2GRAY)

    not_white = (
        (img[:, :, 0] < 245) | (img[:, :, 1] < 245) | (img[:, :, 2] < 245)
    ).astype(np.uint8) * 255

    # 1. Detect uniform colored banners and shaded header cells (closing white text holes first)
    closed = cv2.morphologyEx(not_white, cv2.MORPH_CLOSE, np.ones((25, 19), np.uint8))
    solid = cv2.morphologyEx(closed, cv2.MORPH_OPEN, np.ones((21, 17), np.uint8))
    num_b, b_labels, b_stats, _ = cv2.connectedComponentsWithStats(solid)
    banner_mask = np.zeros_like(gray)
    dark_mask = np.zeros_like(gray)
    banners: list[tuple[float, float, float, float, tuple[float, float, float]]] = []
    for i in range(1, num_b):
        x, y, w, h, area = b_stats[i]
        if w > iw * 0.12 and h >= 16 and area / max(1, w * h) > 0.72:
            patch = img[y : y + h, x : x + w]
            med_u8 = np.median(patch.reshape(-1, 3), axis=0)
            dists = np.linalg.norm(patch.astype(np.float32) - med_u8, axis=2)
            uniform_ratio = float(np.mean(dists <= 22.0))
            med = med_u8 / 255.0
            if uniform_ratio >= 0.65 and not np.all(med > 0.97):
                banners.append(
                    (
                        x * sx,
                        y * sy,
                        (x + w) * sx,
                        (y + h) * sy,
                        (float(med[0]), float(med[1]), float(med[2])),
                    )
                )
                banner_mask[b_labels == i] = 255
                lum = 0.299 * float(med[0]) + 0.587 * float(med[1]) + 0.114 * float(med[2])
                if lum < 0.45:
                    dark_mask[y : y + h, x : x + w] = 255

    # 2. Detect horizontal & vertical rule/table lines outside dark banners
    # (Keep light shaded table headers in bw so internal vertical/horizontal cell borders are erased!)
    bw = cv2.adaptiveThreshold(
        255 - gray, 255, cv2.ADAPTIVE_THRESH_MEAN_C, cv2.THRESH_BINARY, 15, -6
    )
    if np.any(dark_mask):
        bw[dark_mask > 0] = 0

    h_ker = cv2.getStructuringElement(cv2.MORPH_RECT, (max(36, iw // 22), 1))
    v_ker = cv2.getStructuringElement(cv2.MORPH_RECT, (1, max(28, ih // 32)))
    h_raw = cv2.morphologyEx(bw, cv2.MORPH_OPEN, h_ker)
    v_raw = cv2.morphologyEx(bw, cv2.MORPH_OPEN, v_ker)

    line_mask = np.zeros_like(gray)
    h_lines: list[tuple[float, float, float, float, tuple[float, float, float]]] = []
    v_lines: list[tuple[float, float, float, float, tuple[float, float, float]]] = []

    num_h, h_lab, h_st, _ = cv2.connectedComponentsWithStats(h_raw)
    for i in range(1, num_h):
        x, y, w, h, _ = h_st[i]
        if h <= 10 and w >= 30 and w / max(1, h) >= 5.0:
            ym = (y + h * 0.5) * sy
            col = np.median(img[y : y + h, x : x + w].reshape(-1, 3), axis=0) / 255.0
            if not np.all(col > 0.95):
                h_lines.append(
                    (x * sx, ym, (x + w) * sx, max(0.6, h * sy), tuple(float(c) for c in col))
                )
                line_mask[h_lab == i] = 255

    num_v, v_lab, v_st, _ = cv2.connectedComponentsWithStats(v_raw)
    for i in range(1, num_v):
        x, y, w, h, _ = v_st[i]
        if w <= 10 and h >= 22 and h / max(1, w) >= 4.0:
            xm = (x + w * 0.5) * sx
            col = np.median(img[y : y + h, x : x + w].reshape(-1, 3), axis=0) / 255.0
            if not np.all(col > 0.95):
                v_lines.append(
                    (xm, y * sy, (y + h) * sy, max(0.6, w * sx), tuple(float(c) for c in col))
                )
                line_mask[v_lab == i] = 255

    # Recover shaded header horizontal/vertical perimeter edges along banner boundaries cleanly
    if np.any(banner_mask):
        b_edges = cv2.Canny(banner_mask, 50, 150)
        bh_raw = cv2.morphologyEx(b_edges, cv2.MORPH_OPEN, h_ker)
        bv_raw = cv2.morphologyEx(b_edges, cv2.MORPH_OPEN, v_ker)
        for comp_mask, target_list, is_horiz in ((bh_raw, h_lines, True), (bv_raw, v_lines, False)):
            nc, clab, cst, _ = cv2.connectedComponentsWithStats(comp_mask)
            for idx_c in range(1, nc):
                x, y, w, h, _ = cst[idx_c]
                if is_horiz and w >= 40 and h <= 6:
                    ym = (y + h * 0.5) * sy
                    target_list.append((x * sx, ym, (x + w) * sx, 0.75, (0.55, 0.55, 0.58)))
                    line_mask[clab == idx_c] = 255
                elif not is_horiz and h >= 20 and w <= 6:
                    xm = (x + w * 0.5) * sx
                    target_list.append((xm, y * sy, (y + h) * sy, 0.75, (0.55, 0.55, 0.58)))
                    line_mask[clab == idx_c] = 255

    combined_bg = cv2.bitwise_or(banner_mask, cv2.dilate(line_mask, np.ones((5, 5), np.uint8)))
    return banners, h_lines, v_lines, combined_bg, line_mask, dark_mask


def sample_ink_and_bold(
    img: np.ndarray,
    bbox_pt: tuple[float, float, float, float],
    sx: float,
    sy: float,
    on_dark_banner: bool,
) -> tuple[tuple[float, float, float], bool]:
    ih, iw = img.shape[:2]
    px0 = max(0, int(bbox_pt[0] / sx) - 1)
    py0 = max(0, int(bbox_pt[1] / sy) - 1)
    px1 = min(iw, int(bbox_pt[2] / sx) + 2)
    py1 = min(ih, int(bbox_pt[3] / sy) + 2)
    patch = img[py0:py1, px0:px1]
    if patch.shape[0] < 3 or patch.shape[1] < 3:
        return ((1.0, 1.0, 1.0) if on_dark_banner else (0.12, 0.14, 0.18)), False

    border = np.concatenate([patch[0], patch[-1], patch[:, 0], patch[:, -1]], axis=0)
    bg = np.median(border, axis=0)
    dists = np.linalg.norm(patch.astype(np.float32) - bg, axis=2)
    cutoff = max(float(np.percentile(dists, 82)), 20.0)
    fg_mask = dists >= cutoff
    fg_ratio = float(np.mean(fg_mask))
    if np.any(fg_mask):
        ink = np.median(patch[fg_mask], axis=0) / 255.0
    else:
        ink = np.array([0.12, 0.14, 0.18])

    if not on_dark_banner and np.all(ink > 0.85):
        ink = np.array([0.12, 0.14, 0.18])
    return (float(ink[0]), float(ink[1]), float(ink[2])), fg_ratio > 0.34


def split_line_into_chunks(
    line: dict,
    v_lines: list[tuple[float, float, float, float, tuple[float, float, float]]],
) -> list[dict]:
    spans = sorted(line.get("spans", []), key=lambda s: s["bbox"][0])
    if not spans:
        return []
    line_y0, line_y1 = line["bbox"][1], line["bbox"][3]
    line_h = max(6.0, line_y1 - line_y0)

    chunks: list[dict] = []
    cur_text = ""
    cur_bbox = list(spans[0]["bbox"])
    cur_bold = bool(spans[0].get("flags", 0) & 16)
    cur_size = float(spans[0].get("size", line_h * 0.78))
    prev_x1 = spans[0]["bbox"][2]

    for idx, sp in enumerate(spans):
        txt = sp.get("text", "")
        if not txt:
            continue
        bx0, by0, bx1, by1 = sp["bbox"]
        if idx > 0 and cur_text:
            gap = bx0 - prev_x1
            crossed_vline = any(
                (prev_x1 - 2.0) <= vx <= (bx0 + 2.0) and vy0 <= (line_y0 + line_y1) * 0.5 <= vy1
                for vx, vy0, vy1, _, _ in v_lines
            )
            if gap > max(6.0, line_h * 0.9) or crossed_vline:
                if cur_text.strip():
                    chunks.append(
                        {
                            "text": cur_text.strip(),
                            "bbox": tuple(cur_bbox),
                            "bold": cur_bold,
                            "size": cur_size,
                        }
                    )
                cur_text = txt
                cur_bbox = [bx0, by0, bx1, by1]
                cur_bold = bool(sp.get("flags", 0) & 16)
                cur_size = float(sp.get("size", line_h * 0.78))
                prev_x1 = bx1
                continue
            if gap > line_h * 0.18 and not cur_text.endswith(" ") and not txt.startswith(" "):
                cur_text += " "

        cur_text += txt
        cur_bbox[0] = min(cur_bbox[0], bx0)
        cur_bbox[1] = min(cur_bbox[1], by0)
        cur_bbox[2] = max(cur_bbox[2], bx1)
        cur_bbox[3] = max(cur_bbox[3], by1)
        cur_bold = cur_bold or bool(sp.get("flags", 0) & 16)
        cur_size = max(cur_size, float(sp.get("size", line_h * 0.78)))
        prev_x1 = max(prev_x1, bx1)

    if cur_text.strip():
        chunks.append(
            {
                "text": cur_text.strip(),
                "bbox": tuple(cur_bbox),
                "bold": cur_bold,
                "size": cur_size,
            }
        )
    return chunks


def run_hybrid_ocr_on_image(
    img: np.ndarray,
    banners: list[tuple[float, float, float, float, tuple[float, float, float]]],
    line_mask: np.ndarray,
    pw: float,
    ph: float,
    lang: str,
    tessdata_dir: str | None,
) -> dict:
    ih, iw = img.shape[:2]
    sx, sy = pw / iw, ph / ih
    ocr_img = img.copy()

    # Erase table/rule lines cleanly on white so Tesseract doesn't skip bordered table cells
    erased = cv2.dilate(line_mask, np.ones((5, 5), np.uint8))
    ocr_img[erased > 0] = [255, 255, 255]

    # Normalize every uniform banner / shaded table header to black text on pure white paper
    for bx0, by0, bx1, by1, fill_col in banners:
        px0, py0 = max(0, int(bx0 / sx)), max(0, int(by0 / sy))
        px1, py1 = min(iw, int(bx1 / sx)), min(ih, int(by1 / sy))
        if px1 <= px0 or py1 <= py0:
            continue
        sub = img[py0:py1, px0:px1].astype(np.float32)
        bg = np.array(fill_col, dtype=np.float32) * 255.0
        dists = np.linalg.norm(sub - bg, axis=2)
        not_line = erased[py0:py1, px0:px1] == 0
        fg = (dists >= 28.0) & not_line
        lum = 0.299 * fill_col[0] + 0.587 * fill_col[1] + 0.114 * fill_col[2]
        out_sub = np.full_like(img[py0:py1, px0:px1], 255)
        if lum < 0.45:
            out_sub[fg] = np.clip(255.0 - sub[fg], 0, 220).astype(np.uint8)
        else:
            out_sub[fg] = img[py0:py1, px0:px1][fg]
        ocr_img[py0:py1, px0:px1] = out_sub

    ok, png_buf = cv2.imencode(".png", cv2.cvtColor(ocr_img, cv2.COLOR_RGB2BGR))
    if not ok:
        return {"blocks": []}

    tmp = pymupdf.open()
    tp_page = tmp.new_page(width=pw, height=ph)
    tp_page.insert_image(tp_page.rect, stream=png_buf.tobytes())
    kwargs: dict = {"language": lang, "dpi": 250, "full": True}
    if tessdata_dir:
        kwargs["tessdata"] = tessdata_dir
    tp = tp_page.get_textpage_ocr(**kwargs)
    odict = tp_page.get_text("dict", textpage=tp)
    tmp.close()
    return odict


def reconstruct_page(
    src_page: pymupdf.Page,
    out_doc: pymupdf.Document,
    hidden_only: bool,
    lang: str,
    tessdata_dir: str | None,
) -> None:
    pw, ph = float(src_page.rect.width), float(src_page.rect.height)
    pix = src_page.get_pixmap(dpi=250, colorspace=pymupdf.csRGB)
    img = np.frombuffer(pix.samples, dtype=np.uint8).reshape(pix.height, pix.width, 3).copy()
    if not hidden_only:
        skew = estimate_skew_angle(cv2.cvtColor(img, cv2.COLOR_RGB2GRAY))
        if abs(skew) >= 0.35:
            img = rotate_rgb(img, skew)

    ih, iw = img.shape[:2]
    sx, sy = pw / iw, ph / ih
    banners, h_lines, v_lines, bg_mask, line_mask, _dark_mask = detect_banners_and_lines(
        img, pw, ph
    )
    odict = (
        src_page.get_text("dict")
        if hidden_only
        else run_hybrid_ocr_on_image(img, banners, line_mask, pw, ph, lang, tessdata_dir)
    )
    out_page = out_doc.new_page(width=pw, height=ph)

    # 1. Draw solid banners / shaded table headers
    for x0, y0, x1, y1, fill_col in banners:
        out_page.draw_rect(pymupdf.Rect(x0, y0, x1, y1), fill=fill_col, color=None)

    # 2. Extract and split text chunks so table columns stay cleanly separated
    chunks: list[dict] = []
    text_mask = np.zeros((ih, iw), dtype=np.uint8)
    for b in odict.get("blocks", []):
        if b.get("type") != 0:
            continue
        for line in b.get("lines", []):
            for ch in split_line_into_chunks(line, v_lines):
                t_s = ch["text"].strip()
                if len(t_s) <= 2 and not any(c.isalnum() for c in t_s) and t_s not in ("•", "-", "*"):
                    continue
                if len(t_s) <= 3:
                    bx0, by0, bx1, by1 = ch["bbox"]
                    px0, py0 = max(0, int(bx0 / sx)), max(0, int(by0 / sy))
                    px1, py1 = min(iw, int(bx1 / sx)), min(ih, int(by1 / sy))
                    if px1 > px0 and py1 > py0:
                        patch = img[py0:py1, px0:px1]
                        chroma = patch.max(axis=2).astype(np.int16) - patch.min(axis=2).astype(np.int16)
                        if float(np.mean(chroma > 45)) > 0.25:
                            continue
                chunks.append(ch)
                bx0, by0, bx1, by1 = ch["bbox"]
                tx0 = max(0, int(bx0 / sx) - 5)
                ty0 = max(0, int(by0 / sy) - 4)
                tx1 = min(iw, int(bx1 / sx) + 5)
                ty1 = min(ih, int(by1 / sy) + 4)
                text_mask[ty0:ty1, tx0:tx1] = 255

    # 3. Extract non-text graphics / photos / logos (preserving colorful logos sitting on banners)
    not_white = (
        (img[:, :, 0] < 245) | (img[:, :, 1] < 245) | (img[:, :, 2] < 245)
    ).astype(np.uint8) * 255
    fig_mask = not_white.copy()
    fig_mask[bg_mask > 0] = 0
    fig_mask[text_mask > 0] = 0
    # Re-enable high-contrast logos/stamps sitting inside uniform banners (excluding grid lines & text)
    erased_lines = cv2.dilate(line_mask, np.ones((7, 7), np.uint8))
    for bx0, by0, bx1, by1, fill_col in banners:
        px0, py0 = max(0, int(bx0 / sx)), max(0, int(by0 / sy))
        px1, py1 = min(iw, int(bx1 / sx)), min(ih, int(by1 / sy))
        if px1 > px0 and py1 > py0:
            sub = img[py0:py1, px0:px1].astype(np.float32)
            med_bgr = np.array(fill_col, dtype=np.float32) * 255.0
            diff = np.linalg.norm(sub - med_bgr, axis=2)
            banner_fg = (diff >= 50.0).astype(np.uint8) * 255
            banner_fg[text_mask[py0:py1, px0:px1] > 0] = 0
            banner_fg[erased_lines[py0:py1, px0:px1] > 0] = 0
            fig_mask[py0:py1, px0:px1] = cv2.bitwise_or(fig_mask[py0:py1, px0:px1], banner_fg)

    fig_mask = cv2.morphologyEx(fig_mask, cv2.MORPH_CLOSE, np.ones((7, 7), np.uint8))
    fig_mask = cv2.morphologyEx(fig_mask, cv2.MORPH_OPEN, np.ones((3, 3), np.uint8))
    num_f, _, f_stats, _ = cv2.connectedComponentsWithStats(fig_mask)
    for i in range(1, num_f):
        x, y, w, h, area = f_stats[i]
        if area < 600 or w < 22 or h < 22 or w * h > 0.75 * iw * ih:
            continue
        crop = img[y : y + h, x : x + w]
        ok, crop_png = cv2.imencode(".png", cv2.cvtColor(crop, cv2.COLOR_RGB2BGR))
        if ok:
            gbox = (x * sx, y * sy, (x + w) * sx, (y + h) * sy)
            out_page.insert_image(pymupdf.Rect(*gbox), stream=crop_png.tobytes())

    # 4. Draw crisp vector rules & table grid borders
    for x0, y, x1, w_pt, col in h_lines:
        out_page.draw_line((x0, y), (x1, y), color=col, width=w_pt)
    for x, y0, y1, w_pt, col in v_lines:
        out_page.draw_line((x, y0), (x, y1), color=col, width=w_pt)

    # 5. Draw text chunks on top with width-safe font scaling
    reg_ttf = find_unicode_ttf(False)
    bold_ttf = find_unicode_ttf(True)
    for ch in chunks:
        txt = ch["text"]
        x0, y0, x1, y1 = ch["bbox"]
        box_h = max(6.0, y1 - y0)
        box_w = max(8.0, x1 - x0)
        cx_pt, cy_pt = (x0 + x1) * 0.5, (y0 + y1) * 0.5
        on_dark = False
        for bx0, by0, bx1, by1, fcol in banners:
            if bx0 <= cx_pt <= bx1 and by0 <= cy_pt <= by1:
                lum = 0.299 * fcol[0] + 0.587 * fcol[1] + 0.114 * fcol[2]
                if lum < 0.45:
                    on_dark = True
                    break

        ink, pixel_bold = sample_ink_and_bold(img, ch["bbox"], sx, sy, on_dark)
        is_bold = ch["bold"] or pixel_bold
        fs = max(6.0, min(64.0, box_h * 0.78))

        ascii_ok = True
        try:
            txt.encode("latin-1")
        except UnicodeEncodeError:
            ascii_ok = False

        fontname = "hebo" if is_bold else "helv"
        fontfile = None
        if not ascii_ok:
            ttf = bold_ttf if is_bold and bold_ttf else reg_ttf
            if ttf:
                fontname = "custom_b" if is_bold else "custom_r"
                fontfile = ttf

        try:
            measured = pymupdf.get_text_length(
                txt, fontname="helv" if fontfile is None else fontname, fontsize=fs
            )
        except Exception:
            measured = box_w
        if measured > box_w * 1.15 and measured > 0:
            fs = max(5.5, fs * (box_w * 1.10 / measured))

        baseline_y = y1 - box_h * 0.18
        kwargs: dict = {"fontsize": fs, "fontname": fontname, "color": ink}
        if fontfile:
            kwargs["fontfile"] = fontfile
        try:
            out_page.insert_text((x0, baseline_y), txt, **kwargs)
        except Exception:
            out_page.insert_text(
                (x0, baseline_y),
                txt,
                fontsize=fs,
                fontname="hebo" if is_bold else "helv",
                color=ink,
            )


FLOAT_NUM_RE = re.compile(r"^[-+]?\d+\.\d+$")
INT_W_ATTRS = {
    f"{W_TAG}{name}"
    for name in (
        "w",
        "sz",
        "space",
        "before",
        "after",
        "line",
        "left",
        "right",
        "firstLine",
        "hanging",
        "h",
        "top",
        "bottom",
        "header",
        "footer",
        "gutter",
    )
}
INT_VAL_TAGS = {
    f"{W_TAG}{name}"
    for name in ("w", "trHeight", "sz", "szCs", "kern", "position", "gridSpan")
}


def sanitize_int_attrs(root: etree._Element) -> None:
    val_key = f"{W_TAG}val"
    w_key = f"{W_TAG}w"
    for el in root.iter():
        for attr in list(el.attrib):
            raw = el.attrib[attr]
            if not FLOAT_NUM_RE.match(raw):
                continue
            if attr in INT_W_ATTRS or (attr == val_key and el.tag in INT_VAL_TAGS):
                el.set(attr, str(int(round(float(raw)))))
        if el.tag == f"{W_TAG}tblInd" and w_key in el.attrib:
            try:
                num = int(round(float(el.attrib[w_key])))
                if -40 <= num < 0:
                    el.set(w_key, "0")
            except ValueError:
                pass


def parse_dxa_int(el: etree._Element | None, attr: str, default: int = 0) -> int:
    if el is None:
        return default
    raw = el.get(attr)
    if not raw:
        return default
    try:
        return int(round(float(raw)))
    except ValueError:
        return default


def fix_table_grids_and_heights(root: etree._Element) -> None:
    w_attr = f"{W_TAG}w"
    type_attr = f"{W_TAG}type"
    val_attr = f"{W_TAG}val"
    hrule_attr = f"{W_TAG}hRule"

    for tbl in root.iter(f"{W_TAG}tbl"):
        trs = tbl.findall(f"{W_TAG}tr")
        if not trs:
            continue

        row_spans_widths: list[list[tuple[int, int]]] = []
        max_cols = 0
        for tr in trs:
            tr_pr = tr.find(f"{W_TAG}trPr")
            if tr_pr is not None:
                tr_h = tr_pr.find(f"{W_TAG}trHeight")
                if tr_h is not None and tr_h.get(hrule_attr) == "exact":
                    tr_h.set(hrule_attr, "atLeast")
            cells: list[tuple[int, int]] = []
            col_cursor = 0
            for tc in tr.findall(f"{W_TAG}tc"):
                tc_pr = tc.find(f"{W_TAG}tcPr")
                span = 1
                tc_w_val = 0
                if tc_pr is not None:
                    gs = tc_pr.find(f"{W_TAG}gridSpan")
                    if gs is not None:
                        span = max(1, parse_dxa_int(gs, val_attr, 1))
                    tc_w = tc_pr.find(f"{W_TAG}tcW")
                    if tc_w is not None and tc_w.get(type_attr, "dxa") in ("dxa", ""):
                        tc_w_val = max(0, parse_dxa_int(tc_w, w_attr, 0))
                cells.append((span, tc_w_val))
                col_cursor += span
            max_cols = max(max_cols, col_cursor)
            row_spans_widths.append(cells)

        tbl_grid = tbl.find(f"{W_TAG}tblGrid")
        existing_grid = (
            [parse_dxa_int(gc, w_attr, 0) for gc in tbl_grid.findall(f"{W_TAG}gridCol")]
            if tbl_grid is not None
            else []
        )
        max_cols = max(max_cols, len(existing_grid))
        if max_cols == 0:
            continue

        col_widths = [0] * max_cols
        for cells in row_spans_widths:
            c_idx = 0
            for span, tc_w_val in cells:
                if span == 1 and tc_w_val > 0 and c_idx < max_cols:
                    col_widths[c_idx] = max(col_widths[c_idx], tc_w_val)
                c_idx += span

        for cells in row_spans_widths:
            c_idx = 0
            for span, tc_w_val in cells:
                if span > 1 and tc_w_val > 0 and c_idx + span <= max_cols:
                    missing = [k for k in range(c_idx, c_idx + span) if col_widths[k] == 0]
                    if missing:
                        known = sum(col_widths[k] for k in range(c_idx, c_idx + span))
                        rem = max(360 * len(missing), tc_w_val - known)
                        share = max(360, rem // len(missing))
                        for k in missing:
                            col_widths[k] = share
                c_idx += span

        for k in range(max_cols):
            if col_widths[k] <= 0:
                col_widths[k] = existing_grid[k] if k < len(existing_grid) and existing_grid[k] > 0 else 1440

        if tbl_grid is None:
            tbl_grid = etree.Element(f"{W_TAG}tblGrid")
            tbl_pr = tbl.find(f"{W_TAG}tblPr")
            insert_pos = tbl.index(tbl_pr) + 1 if tbl_pr is not None else 0
            tbl.insert(insert_pos, tbl_grid)
        else:
            tbl_grid.clear()
        for w in col_widths:
            gc = etree.SubElement(tbl_grid, f"{W_TAG}gridCol")
            gc.set(w_attr, str(w))

        tbl_pr = tbl.find(f"{W_TAG}tblPr")
        if tbl_pr is None:
            tbl_pr = etree.Element(f"{W_TAG}tblPr")
            tbl.insert(0, tbl_pr)
        tbl_w = tbl_pr.find(f"{W_TAG}tblW")
        if tbl_w is None:
            tbl_w = etree.SubElement(tbl_pr, f"{W_TAG}tblW")
        tbl_w.set(w_attr, str(sum(col_widths)))
        tbl_w.set(type_attr, "dxa")
        tbl_layout = tbl_pr.find(f"{W_TAG}tblLayout")
        if tbl_layout is None:
            tbl_layout = etree.SubElement(tbl_pr, f"{W_TAG}tblLayout")
        tbl_layout.set(type_attr, "fixed")


def relax_exact_line_spacing(root: etree._Element) -> None:
    lr_attr = f"{W_TAG}lineRule"
    for p in root.iter(f"{W_TAG}p"):
        has_text = any((t.text or "").strip() for t in p.iter(f"{W_TAG}t"))
        if not has_text:
            continue
        p_pr = p.find(f"{W_TAG}pPr")
        if p_pr is None:
            continue
        sp = p_pr.find(f"{W_TAG}spacing")
        if sp is not None and sp.get(lr_attr) == "exact":
            sp.set(lr_attr, "atLeast")


def _max_sz_half_pt(nodes: list[etree._Element], default: int = 22) -> int:
    sz_tag = f"{W_TAG}sz"
    val_attr = f"{W_TAG}val"
    best = 0
    for node in nodes:
        for sz in node.iter(sz_tag):
            v = parse_dxa_int(sz, val_attr, 0)
            if v > best:
                best = v
    return best if best > 0 else default


def split_heading_soft_breaks(root: etree._Element) -> None:
    br_tag = f"{W_TAG}br"
    r_tag = f"{W_TAG}r"
    type_attr = f"{W_TAG}type"
    ppr_tag = f"{W_TAG}pPr"
    sect_tag = f"{W_TAG}sectPr"

    for p in list(root.iter(f"{W_TAG}p")):
        parent = p.getparent()
        if parent is None:
            continue
        p_pr = p.find(ppr_tag)
        body_children = [ch for ch in p if ch.tag != ppr_tag]
        # Build segments separated by soft <w:br/> elements
        segments: list[list[etree._Element]] = [[]]
        for ch in body_children:
            if ch.tag != r_tag:
                segments[-1].append(ch)
                continue
            soft_brs = [
                br for br in ch.findall(br_tag) if br.get(type_attr) not in ("page", "column")
            ]
            if not soft_brs:
                segments[-1].append(ch)
                continue
            # Split run at soft <w:br/> nodes
            r_pr = ch.find(f"{W_TAG}rPr")
            r_children = [rc for rc in ch if rc.tag != f"{W_TAG}rPr"]
            cur_r_children: list[etree._Element] = []
            for rc in r_children:
                if rc.tag == br_tag and rc.get(type_attr) not in ("page", "column"):
                    if cur_r_children:
                        new_r = etree.Element(r_tag, attrib=dict(ch.attrib))
                        if r_pr is not None:
                            new_r.append(copy.deepcopy(r_pr))
                        for item in cur_r_children:
                            new_r.append(copy.deepcopy(item))
                        segments[-1].append(new_r)
                        cur_r_children = []
                    segments.append([])
                else:
                    cur_r_children.append(rc)
            if cur_r_children:
                new_r = etree.Element(r_tag, attrib=dict(ch.attrib))
                if r_pr is not None:
                    new_r.append(copy.deepcopy(r_pr))
                for item in cur_r_children:
                    new_r.append(copy.deepcopy(item))
                segments[-1].append(new_r)

        non_empty = [seg for seg in segments if any((t.text or "").strip() for n in seg for t in n.iter(f"{W_TAG}t")) or any(n.find(f".//{W_TAG}drawing") is not None for n in seg)]
        if len(non_empty) <= 1:
            continue

        # Merge adjacent segments unless font size changes >= 4 half-points (2 pt)
        grouped: list[list[etree._Element]] = [list(segments[0])]
        prev_sz = _max_sz_half_pt(segments[0])
        for seg in segments[1:]:
            if not seg:
                continue
            cur_sz = _max_sz_half_pt(seg, prev_sz)
            has_txt_prev = any((t.text or "").strip() for n in grouped[-1] for t in n.iter(f"{W_TAG}t"))
            has_txt_cur = any((t.text or "").strip() for n in seg for t in n.iter(f"{W_TAG}t"))
            if has_txt_prev and has_txt_cur and abs(prev_sz - cur_sz) >= 4:
                grouped.append(list(seg))
                prev_sz = cur_sz
            else:
                if has_txt_prev and has_txt_cur:
                    br_r = etree.Element(r_tag)
                    etree.SubElement(br_r, br_tag)
                    grouped[-1].append(br_r)
                grouped[-1].extend(seg)
                prev_sz = cur_sz

        if len(grouped) <= 1:
            continue

        idx = parent.index(p)
        for gi, grp in enumerate(grouped):
            new_p = etree.Element(f"{W_TAG}p", attrib=dict(p.attrib))
            if p_pr is not None:
                cloned_pr = copy.deepcopy(p_pr)
                if gi + 1 < len(grouped):
                    sp = cloned_pr.find(sect_tag)
                    if sp is not None:
                        cloned_pr.remove(sp)
                new_p.append(cloned_pr)
            for node in grp:
                new_p.append(copy.deepcopy(node))
            parent.insert(idx + gi, new_p)
        parent.remove(p)


def _borders_are_invisible(borders_el: etree._Element | None) -> bool:
    if borders_el is None:
        return True
    val_attr = f"{W_TAG}val"
    sz_attr = f"{W_TAG}sz"
    for side in borders_el:
        val = (side.get(val_attr) or "none").lower()
        sz = parse_dxa_int(side, sz_attr, 0)
        if val not in ("none", "nil") and sz > 0:
            return False
    return True


def _cell_has_visible_shd(tc_pr: etree._Element | None) -> bool:
    if tc_pr is None:
        return False
    shd = tc_pr.find(f"{W_TAG}shd")
    if shd is None:
        return False
    val = (shd.get(f"{W_TAG}val") or "clear").lower()
    fill = (shd.get(f"{W_TAG}fill") or "auto").lower()
    return val != "nil" and fill not in ("auto", "ffffff")


def _inline_to_anchor(inline: etree._Element, pos_x_emu: int) -> tuple[etree._Element, int]:
    ext = inline.find(f"{WP_TAG}extent")
    cx = ext.get("cx", "1143000") if ext is not None else "1143000"
    cy_str = ext.get("cy", "1143000") if ext is not None else "1143000"
    cy_val = int(cy_str)

    anchor = etree.Element(
        f"{WP_TAG}anchor",
        {
            "distT": "0",
            "distB": "0",
            "distL": "114300",
            "distR": "114300",
            "simplePos": "0",
            "relativeHeight": "251658240",
            "behindDoc": "0",
            "locked": "0",
            "layoutInCell": "1",
            "allowOverlap": "1",
        },
    )
    sp = etree.SubElement(anchor, f"{WP_TAG}simplePos")
    sp.set("x", "0")
    sp.set("y", "0")
    ph = etree.SubElement(anchor, f"{WP_TAG}positionH")
    ph.set("relativeFrom", "column")
    etree.SubElement(ph, f"{WP_TAG}posOffset").text = str(max(0, pos_x_emu))
    pv = etree.SubElement(anchor, f"{WP_TAG}positionV")
    pv.set("relativeFrom", "paragraph")
    etree.SubElement(pv, f"{WP_TAG}posOffset").text = "0"
    ex = etree.SubElement(anchor, f"{WP_TAG}extent")
    ex.set("cx", str(cx))
    ex.set("cy", str(cy_str))
    eff = etree.SubElement(anchor, f"{WP_TAG}effectExtent")
    for k in ("l", "t", "r", "b"):
        eff.set(k, "0")
    etree.SubElement(anchor, f"{WP_TAG}wrapNone")

    for child_name in ("docPr", "cNvGraphicFramePr"):
        src_child = inline.find(f"{WP_TAG}{child_name}")
        if src_child is not None:
            anchor.append(copy.deepcopy(src_child))
        else:
            if child_name == "docPr":
                dp = etree.SubElement(anchor, f"{WP_TAG}docPr")
                dp.set("id", "1")
                dp.set("name", "Picture")
            else:
                cnv = etree.SubElement(anchor, f"{WP_TAG}cNvGraphicFramePr")
                locks = etree.SubElement(cnv, f"{A_TAG}graphicFrameLocks")
                locks.set("noChangeAspect", "1")
    graphic = inline.find(f"{A_TAG}graphic")
    if graphic is not None:
        anchor.append(copy.deepcopy(graphic))
    return anchor, cy_val


def unwrap_photo_header_tables(body: etree._Element) -> None:
    tbl_tag = f"{W_TAG}tbl"
    p_tag = f"{W_TAG}p"
    w_attr = f"{W_TAG}w"

    for tbl in list(body):
        if tbl.tag != tbl_tag:
            continue
        trs = tbl.findall(f"{W_TAG}tr")
        if len(trs) != 1:
            continue
        tcs = trs[0].findall(f"{W_TAG}tc")
        if len(tcs) != 2:
            continue
        if any(tc.find(f".//{tbl_tag}") is not None for tc in tcs):
            continue

        tbl_pr = tbl.find(f"{W_TAG}tblPr")
        tbl_borders = tbl_pr.find(f"{W_TAG}tblBorders") if tbl_pr is not None else None
        if not _borders_are_invisible(tbl_borders):
            continue

        spans_ok = True
        cell_widths = [0, 0]
        cell_texts = ["", ""]
        cell_inlines: list[list[etree._Element]] = [[], []]
        for idx_c, tc in enumerate(tcs):
            tc_pr = tc.find(f"{W_TAG}tcPr")
            if tc_pr is not None:
                gs = tc_pr.find(f"{W_TAG}gridSpan")
                if gs is not None and parse_dxa_int(gs, f"{W_TAG}val", 1) != 1:
                    spans_ok = False
                if not _borders_are_invisible(tc_pr.find(f"{W_TAG}tcBorders")) or _cell_has_visible_shd(tc_pr):
                    spans_ok = False
                cell_widths[idx_c] = parse_dxa_int(tc_pr.find(f"{W_TAG}tcW"), w_attr, 0)
            cell_texts[idx_c] = "".join((t.text or "") for t in tc.iter(f"{W_TAG}t")).strip()
            cell_inlines[idx_c] = list(tc.iter(f"{WP_TAG}inline"))
            if list(tc.iter(f"{WP_TAG}anchor")):
                spans_ok = False

        if not spans_ok:
            continue

        if cell_texts[0] == "" and len(cell_inlines[0]) == 1 and cell_texts[1] != "":
            img_idx = 0
        elif cell_texts[1] == "" and len(cell_inlines[1]) == 1 and cell_texts[0] != "":
            img_idx = 1
        else:
            continue

        txt_idx = 1 - img_idx
        col0_w = cell_widths[0] or 1980
        col1_w = cell_widths[1] or 8880

        # Determine image paragraph horizontal indent inside img_cell
        inline_el = cell_inlines[img_idx][0]
        img_p = inline_el
        while img_p is not None and img_p.tag != p_tag:
            img_p = img_p.getparent()
        img_ind = 0
        if img_p is not None:
            ipr = img_p.find(f"{W_TAG}pPr")
            if ipr is not None:
                img_ind = parse_dxa_int(ipr.find(f"{W_TAG}ind"), f"{W_TAG}left", 0)
        pos_x_emu = (img_ind if img_idx == 0 else col0_w + img_ind) * 635
        anchor_el, cy_emu = _inline_to_anchor(inline_el, pos_x_emu)

        promoted: list[etree._Element] = []
        for raw_p in tcs[txt_idx].findall(p_tag):
            has_txt = any((t.text or "").strip() for t in raw_p.iter(f"{W_TAG}t"))
            has_draw = raw_p.find(f".//{W_TAG}drawing") is not None
            if not has_txt and not has_draw:
                continue
            new_p = copy.deepcopy(raw_p)
            p_pr = new_p.find(f"{W_TAG}pPr")
            if p_pr is None:
                p_pr = etree.Element(f"{W_TAG}pPr")
                new_p.insert(0, p_pr)
            ind = p_pr.find(f"{W_TAG}ind")
            if ind is None:
                ind = etree.SubElement(p_pr, f"{W_TAG}ind")
            if img_idx == 0:
                cur_l = parse_dxa_int(ind, f"{W_TAG}left", 0)
                ind.set(f"{W_TAG}left", str(max(0, cur_l + col0_w)))
            else:
                cur_r = parse_dxa_int(ind, f"{W_TAG}right", 0)
                ind.set(f"{W_TAG}right", str(max(0, cur_r + col1_w)))
            promoted.append(new_p)

        if not promoted:
            continue

        # Merge orphan continuation paragraph right below table (e.g. "development.")
        if img_idx == 0:
            next_el = tbl.getnext()
            last_p = promoted[-1]
            if (
                next_el is not None
                and next_el.tag == p_tag
                and next_el.find(f".//{W_TAG}drawing") is None
                and next_el.find(f".//{W_TAG}sectPr") is None
                and not any(
                    br.get(f"{W_TAG}type") in ("page", "column")
                    for br in next_el.iter(f"{W_TAG}br")
                )
            ):
                next_txt = "".join((t.text or "") for t in next_el.iter(f"{W_TAG}t")).lstrip()
                if next_txt and next_txt[0].islower():
                    n_pr = next_el.find(f"{W_TAG}pPr")
                    l_pr = last_p.find(f"{W_TAG}pPr")
                    n_left = parse_dxa_int(n_pr.find(f"{W_TAG}ind") if n_pr is not None else None, f"{W_TAG}left", 0)
                    l_left = parse_dxa_int(l_pr.find(f"{W_TAG}ind") if l_pr is not None else None, f"{W_TAG}left", 0)
                    if abs(n_left - l_left) <= 350:
                        last_txt = "".join((t.text or "") for t in last_p.iter(f"{W_TAG}t"))
                        if last_txt and not last_txt[-1].isspace():
                            sp_r = etree.SubElement(last_p, f"{W_TAG}r")
                            last_runs = last_p.findall(f"{W_TAG}r")
                            if len(last_runs) >= 2:
                                rpr_src = last_runs[-2].find(f"{W_TAG}rPr")
                                if rpr_src is not None:
                                    sp_r.append(copy.deepcopy(rpr_src))
                            sp_t = etree.SubElement(sp_r, f"{W_TAG}t")
                            sp_t.set("{http://www.w3.org/XML/1998/namespace}space", "preserve")
                            sp_t.text = " "
                        for child in next_el:
                            if child.tag != f"{W_TAG}pPr":
                                last_p.append(copy.deepcopy(child))
                        body.remove(next_el)

        # Ensure vertical clearance below photo so following headings never overlap photo
        txt_col_pt = max(140.0, (col1_w if img_idx == 0 else col0_w) / 20.0)
        est_text_h_tw = 0
        for pr_p in promoted:
            p_txt = "".join((t.text or "") for t in pr_p.iter(f"{W_TAG}t"))
            max_sz = _max_sz_half_pt([pr_p], 20)
            font_pt = max_sz / 2.0
            lines = max(1, math.ceil(len(p_txt) * (font_pt * 0.52) / txt_col_pt)) if p_txt.strip() else 1
            sp = pr_p.find(f"{W_TAG}pPr/{W_TAG}spacing")
            line_tw = max(int(font_pt * 24), parse_dxa_int(sp, f"{W_TAG}line", 240))
            bef_tw = parse_dxa_int(sp, f"{W_TAG}before", 0)
            aft_tw = parse_dxa_int(sp, f"{W_TAG}after", 0)
            est_text_h_tw += lines * line_tw + bef_tw + aft_tw

        img_h_tw = cy_emu // 635
        if img_h_tw > est_text_h_tw:
            last_p = promoted[-1]
            last_pr = last_p.find(f"{W_TAG}pPr")
            sp = last_pr.find(f"{W_TAG}spacing") if last_pr is not None else None
            if sp is None and last_pr is not None:
                sp = etree.SubElement(last_pr, f"{W_TAG}spacing")
            if sp is not None:
                cur_aft = parse_dxa_int(sp, f"{W_TAG}after", 0)
                sp.set(f"{W_TAG}after", str(cur_aft + (img_h_tw - est_text_h_tw) + 120))

        # Prepend floating anchor drawing inside first promoted paragraph right after <w:pPr>
        draw_r = etree.Element(f"{W_TAG}r")
        draw_el = etree.SubElement(draw_r, f"{W_TAG}drawing")
        draw_el.append(anchor_el)
        promoted[0].insert(1, draw_r)

        tbl_idx = body.index(tbl)
        for pi, new_p in enumerate(promoted):
            body.insert(tbl_idx + pi, new_p)
        body.remove(tbl)


def normalize_font_name(raw: str) -> str:
    cleaned = SUBSET_PREFIX_RE.sub("", raw.strip())
    cleaned = STYLE_SUFFIX_RE.sub("", cleaned).strip()
    key = re.sub(r"[^a-z0-9]", "", cleaned.lower())
    if key in FONT_ALIAS_MAP:
        return FONT_ALIAS_MAP[key]
    if key.startswith("cidfont") or not cleaned:
        return "Calibri"
    return cleaned


def is_empty_trailing_para(elem: etree._Element) -> bool:
    if elem.tag != f"{W_TAG}p":
        return False
    for node in elem.iter():
        if node.tag in (f"{W_TAG}sectPr", f"{W_TAG}drawing", f"{W_TAG}pict", f"{W_TAG}br"):
            return False
        if node.tag == f"{W_TAG}t" and (node.text or "").strip():
            return False
    return True


def polish_docx(docx_path: pathlib.Path) -> None:
    parser = etree.XMLParser(resolve_entities=False, no_network=True, remove_blank_text=False)
    with zipfile.ZipFile(docx_path, "r") as zin:
        entries = [(info, zin.read(info.filename)) for info in zin.infolist()]

    updated: list[tuple[zipfile.ZipInfo, bytes]] = []
    for info, data in entries:
        if info.filename in ("word/document.xml", "word/styles.xml"):
            try:
                root = etree.fromstring(data, parser=parser)
                for rfonts in root.iter(f"{W_TAG}rFonts"):
                    for attr in ("ascii", "hAnsi", "cs", "eastAsia"):
                        val = rfonts.get(f"{W_TAG}{attr}")
                        if val:
                            rfonts.set(f"{W_TAG}{attr}", normalize_font_name(val))
                sanitize_int_attrs(root)
                if info.filename == "word/document.xml":
                    fix_table_grids_and_heights(root)
                    relax_exact_line_spacing(root)
                    split_heading_soft_breaks(root)
                    body = root.find(f"{W_TAG}body")
                    if body is not None:
                        unwrap_photo_header_tables(body)
                        children = list(body)
                        sect_idx = next(
                            (i for i in range(len(children) - 1, -1, -1) if children[i].tag == f"{W_TAG}sectPr"),
                            -1,
                        )
                        if sect_idx > 1:
                            idx = sect_idx - 1
                            while idx > 0 and is_empty_trailing_para(children[idx]):
                                body.remove(children[idx])
                                idx -= 1
                data = etree.tostring(root, xml_declaration=True, encoding="UTF-8", standalone="yes")
            except Exception:
                pass
        elif info.filename == "word/settings.xml":
            try:
                root = etree.fromstring(data, parser=parser)
                compat = root.find(f"{W_TAG}compat")
                if compat is None:
                    compat = etree.SubElement(root, f"{W_TAG}compat")
                has_mode = any(
                    cs.get(f"{W_TAG}name") == "compatibilityMode"
                    for cs in compat.findall(f"{W_TAG}compatSetting")
                )
                if not has_mode:
                    cs = etree.SubElement(compat, f"{W_TAG}compatSetting")
                    cs.set(f"{W_TAG}name", "compatibilityMode")
                    cs.set(f"{W_TAG}uri", "http://schemas.microsoft.com/office/word")
                    cs.set(f"{W_TAG}val", "15")
                data = etree.tostring(root, xml_declaration=True, encoding="UTF-8", standalone="yes")
            except Exception:
                pass
        updated.append((info, data))

    with zipfile.ZipFile(docx_path, "w", compression=zipfile.ZIP_DEFLATED) as zout:
        for info, data in updated:
            zout.writestr(info.filename, data)


def convert_pdf_to_word(
    input_pdf: pathlib.Path,
    output_docx: pathlib.Path,
    mode: str,
    lang: str,
) -> None:
    force_ocr = mode == "ocr"
    tessdata_dir, resolved_lang = find_tessdata_and_lang(lang)

    src = pymupdf.open(str(input_pdf))
    try:
        classes = [classify_page(src[i], force_ocr) for i in range(len(src))]
        all_digital = all(c == "digital" for c in classes)

        with tempfile.TemporaryDirectory() as tmp_dir:
            if all_digital:
                work_pdf = input_pdf
            else:
                work_pdf = pathlib.Path(tmp_dir) / "preprocessed.pdf"
                out_doc = pymupdf.open()
                for idx, kind in enumerate(classes):
                    if kind == "digital":
                        out_doc.insert_pdf(src, from_page=idx, to_page=idx)
                    else:
                        reconstruct_page(
                            src[idx],
                            out_doc,
                            hidden_only=(kind == "hidden_ocr"),
                            lang=resolved_lang,
                            tessdata_dir=tessdata_dir,
                        )
                out_doc.save(str(work_pdf))
                out_doc.close()

            src.close()
            cv = Converter(str(work_pdf))
            try:
                cv.convert(
                    str(output_docx),
                    start=0,
                    end=None,
                    connected_border_tolerance=1.0,
                    max_border_width=8.0,
                    min_border_clearance=1.5,
                    float_image_ignorable_gap=4.0,
                    page_margin_factor_top=0.35,
                    page_margin_factor_bottom=0.35,
                    delete_end_line_hyphen=True,
                    new_paragraph_free_space_ratio=0.7,
                    parse_lattice_table=True,
                    parse_stream_table=True,
                    list_not_table=True,
                    clip_image_res_ratio=3.0,
                )
            finally:
                cv.close()
    finally:
        if not src.is_closed:
            src.close()

    polish_docx(output_docx)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("input_pdf")
    ap.add_argument("output_docx")
    ap.add_argument("--mode", default="auto", choices=("auto", "ocr"))
    ap.add_argument("--lang", default="ind+eng")
    args = ap.parse_args()
    convert_pdf_to_word(
        pathlib.Path(args.input_pdf),
        pathlib.Path(args.output_docx),
        args.mode,
        args.lang,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
