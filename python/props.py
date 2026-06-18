"""Date arithmetic shared by the Python BI queries.

Mirrors ``rustychickpeas-ldbc/src/props.rs`` so the derived ``day``/``year``
properties match the Rust loader's values exactly (the query results then line
up for cross-checking).
"""


def days_from_civil(y: int, m: int, d: int) -> int:
    """Days since 1970-01-01 for a proleptic-Gregorian date (Hinnant's algorithm).

    Valid for ``y >= 0`` (all LDBC dates), where floor and truncating division
    agree, so Python ``//`` matches Rust's ``/``.
    """
    y = y - 1 if m <= 2 else y
    era = (y if y >= 0 else y - 399) // 400
    yoe = y - era * 400
    doy = (153 * (m - 3 if m > 2 else m + 9) + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def parse_date(s: str):
    """Parse an LDBC creationDate ("2010-02-24T08:06:02.996+00:00") into
    ``(year, days_since_epoch)``; ``None`` if too short / non-numeric."""
    if len(s) < 10:
        return None
    try:
        y = int(s[0:4])
        m = int(s[5:7])
        d = int(s[8:10])
    except ValueError:
        return None
    return (y, days_from_civil(y, m, d))
