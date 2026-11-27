#!/usr/bin/env python3
"""Generate a self-hosted cumulative GitHub stars chart for README.md."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import os
import urllib.request
from pathlib import Path


def fetch_stars(repository: str, token: str | None) -> list[dt.date]:
    url = f"https://api.github.com/repos/{repository}/stargazers?per_page=100"
    dates: list[dt.date] = []
    while url:
        headers = {
            "Accept": "application/vnd.github.star+json",
            "User-Agent": "520pdf-star-history",
            "X-GitHub-Api-Version": "2022-11-28",
        }
        if token:
            headers["Authorization"] = f"Bearer {token}"
        request = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(request, timeout=20) as response:
            for star in json.load(response):
                dates.append(
                    dt.datetime.fromisoformat(star["starred_at"].replace("Z", "+00:00")).date()
                )
            links = response.headers.get("Link", "")
        url = ""
        for link in links.split(","):
            if 'rel="next"' in link:
                url = link[link.index("<") + 1 : link.index(">")]
                break
    return dates


def week_start(day: dt.date) -> dt.date:
    """Return the Monday containing ``day``."""
    return day - dt.timedelta(days=day.weekday())


def render_svg(repository: str, dates: list[dt.date], today: dt.date | None = None) -> str:
    today = today or dt.date.today()
    sorted_dates = sorted(dates)
    current_week = week_start(today)
    weeks = [current_week - dt.timedelta(weeks=w) for w in reversed(range(26))]
    values = [
        sum(day <= min(week + dt.timedelta(days=6), today) for day in sorted_dates)
        for week in weeks
    ]
    weekly = [
        sum(week <= day <= min(week + dt.timedelta(days=6), today) for day in sorted_dates)
        for week in weeks
    ]

    width, height = 800, 420
    left, right, top, bottom = 68, 24, 78, 64
    plot_w, plot_h = width - left - right, height - top - bottom
    latest = values[-1]

    if latest == 0:
        step = 1
        grid_max = 4
    else:
        magnitude = 10 ** max(0, len(str(latest)) - 2)
        step = max(magnitude, math.ceil(latest / 4 / magnitude) * magnitude)
        grid_max = max(step * 4, 4)

    def x(index: int) -> float:
        return left + index * plot_w / (len(weeks) - 1)

    def y(value: int) -> float:
        return top + (1 - value / grid_max) * plot_h

    y_ticks = []
    for index in range(5):
        value = step * index
        yy = y(value)
        label = f"{value / 1000:g}k" if value >= 1000 else str(value)
        y_ticks.append(
            f'<line x1="{left}" y1="{yy:.1f}" x2="{width - right}" y2="{yy:.1f}" class="grid"/>'
            f'<text x="{left - 12}" y="{yy + 5:.1f}" text-anchor="end">{label}</text>'
        )

    points = " ".join(f"{x(index):.1f},{y(value):.1f}" for index, value in enumerate(values))
    area = f"{left},{top + plot_h:.1f} {points} {width - right},{top + plot_h:.1f}"
    x_ticks = []
    dots = []
    for index, (week, value, gain) in enumerate(zip(weeks, values, weekly)):
        if index % 4 == 0 or index == len(weeks) - 1:
            x_ticks.append(
                f'<text x="{x(index):.1f}" y="{height - 30}" text-anchor="middle">'
                f"{week.strftime('%b')} {week.day}</text>"
            )
        current = " current" if week == current_week else ""
        radius = 5 if current else 3
        dots.append(
            f'<circle class="point{current}" cx="{x(index):.1f}" cy="{y(value):.1f}" r="{radius}">'
            f"<title>{week.strftime('%b')} {week.day}: {value:,} total stars (+{gain:,} that week)</title></circle>"
        )

    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
<title id="title">{repository} cumulative GitHub stars over time</title>
<desc id="desc">Cumulative GitHub stars sampled weekly over the last 26 weeks, ending at {latest:,} stars.</desc>
<style>
  :root {{ color-scheme: light dark; }}
  .bg {{ fill: #ffffff; stroke: #e4e4e7; stroke-width: 1; }}
  text {{ fill: #71717a; font: 12px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }}
  .grid {{ stroke: #e4e4e7; stroke-width: 1; }}
  .area {{ fill: #e63228; fill-opacity: .12; }}
  .line {{ fill: none; stroke: #e63228; stroke-width: 2.75; stroke-linejoin: round; stroke-linecap: round; }}
  .point {{ fill: #ffffff; stroke: #e63228; stroke-width: 2; }}
  .point.current {{ fill: #e63228; stroke: #ffffff; stroke-width: 2; }}
  .heading {{ fill: #09090b; font-size: 16px; font-weight: 600; }}
  .metric {{ fill: #09090b; font-size: 17px; font-weight: 600; }}
  @media (prefers-color-scheme: dark) {{
    .bg {{ fill: #0d1117; stroke: #30363d; }}
    text {{ fill: #8b949e; }}
    .grid {{ stroke: #21262d; }}
    .area {{ fill: #ff4d42; fill-opacity: .16; }}
    .line {{ stroke: #ff4d42; }}
    .point {{ fill: #0d1117; stroke: #ff4d42; }}
    .point.current {{ fill: #ff4d42; stroke: #0d1117; }}
    .heading,.metric {{ fill: #f4f4f5; }}
  }}
</style>
<rect class="bg" x="0.5" y="0.5" width="{width - 1}" height="{height - 1}" rx="10"/>
<text class="heading" x="{left}" y="30">GitHub stars over time</text>
<text x="{left}" y="51">Cumulative stars · weekly sampling · last 26 weeks</text>
<text class="metric" x="{width - right}" y="30" text-anchor="end">{latest:,} stars</text>
<text x="{width - right}" y="51" text-anchor="end">+{weekly[-1]:,} this week so far</text>
{''.join(y_ticks)}
<polygon class="area" points="{area}"/>
<polyline class="line" points="{points}"/>
{''.join(dots)}
{''.join(x_ticks)}
</svg>
'''


def _self_test() -> None:
    empty_svg = render_svg("aphelion31/520pdf", [], dt.date(2026, 11, 27))
    assert "<svg " in empty_svg and "0 stars" in empty_svg
    sample = [dt.date(2026, 10, 1), dt.date(2026, 11, 20), dt.date(2026, 11, 26)]
    pop_svg = render_svg("aphelion31/520pdf", sample, dt.date(2026, 11, 27))
    assert "<svg " in pop_svg and "3 stars" in pop_svg and "+1 this week so far" in pop_svg


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default="aphelion31/520pdf")
    parser.add_argument("--output", type=Path, default=Path("docs/images/star-history.svg"))
    parser.add_argument("--allow-anonymous", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        _self_test()
        print("star-history self-test passed")
        return

    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or None
    if not token and not args.allow_anonymous:
        raise SystemExit("GITHUB_TOKEN or GH_TOKEN is required (or pass --allow-anonymous)")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(render_svg(args.repo, fetch_stars(args.repo, token)))
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
