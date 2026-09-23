"""DEVELOPMENT_EVOLUTION only: bounded continuation policy, no product authority."""
import datetime as dt
import json
import re
import zoneinfo

DEFAULT_TURN_THRESHOLD = 24
MAX_HANDOFF_BYTES = 8192
MIN_RETRY_SECONDS = 300


def compact_handoff(block, protected_ok, *, desk_escalation=False):
    """Reject ambiguity/oversize rather than silently discard decision context."""
    if not desk_escalation and any(token in str(block.get("reason", "")).lower() for token in
           ("founder_decision_required", "desk_escalation_required", "authority_boundary")):
        raise ValueError("unresolved authority boundary")
    h = block.get("handoff")
    if not isinstance(h, dict) or block.get("status") != ("stop" if desk_escalation else "continue"):
        raise ValueError("missing continuation handoff")
    if h.get("boundary") not in ("increment", "submilestone"):
        raise ValueError("not an increment boundary")
    for key in ("committed", "review_queued", "report_queued"):
        if type(h.get(key)) is not bool or (not desk_escalation and h[key] is not True):
            raise ValueError("incomplete boundary: " + key)
    if (type(h.get("authority_pending")) is not bool or type(h.get("ambiguous_state")) is not bool
            or (not desk_escalation and (h["authority_pending"] or h["ambiguous_state"]))):
        raise ValueError("unresolved authority or ambiguous state")
    if not protected_ok:
        raise ValueError("protected worktree breach")
    result = {"schema": "maia.night_shift_handoff.v1",
              "protected_worktree_status": "verified_preserved"}
    for key, value in (("work_item", block.get("work_item")),
                       ("next_action", block.get("next_action")),
                       ("increment", h.get("increment"))):
        if not isinstance(value, str) or not value.strip() or len(value) > 1024:
            raise ValueError("invalid " + key)
        result[key] = value
    for key in ("recent_commits", "unresolved_review_findings", "architectural_constraints"):
        value = h.get(key)
        if (not isinstance(value, list) or len(value) > 12
                or any(not isinstance(v, str) or len(v) > 512 for v in value)):
            raise ValueError("invalid " + key)
        result[key] = value
    if not result["recent_commits"] or not result["architectural_constraints"]:
        raise ValueError("missing durable evidence or constraints")
    if desk_escalation:
        if any(not re.fullmatch(r"[0-9a-f]{40}", v) for v in result["recent_commits"]):
            raise ValueError("full commit hashes required")
        evidence = h.get("material_evidence")
        if not isinstance(evidence, str) or not evidence.strip() or len(evidence) > 2048:
            raise ValueError("inline material_evidence required")
        if re.match(r"^(?:[A-Za-z]:[\\/]|\\\\|(?:reports|docs)/)\S*$", evidence.strip()):
            raise ValueError("local path alone is not inline material evidence")
        if any(not v.strip() for key in ("unresolved_review_findings", "architectural_constraints")
               for v in result[key]):
            raise ValueError("empty inline findings or constraints")
        result.update({key: h[key] for key in ("boundary", "committed", "review_queued",
                       "report_queued", "authority_pending", "ambiguous_state")})
        result["material_evidence"] = evidence
    encoded = json.dumps(result, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    if len(encoded.encode("utf-8")) > MAX_HANDOFF_BYTES:
        raise ValueError("handoff exceeds byte budget; compact without losing decisions")
    return encoded


def verification_policy(paths, boundary="increment", safety_critical=False):
    """Promotion evidence is never substituted by targeted tests."""
    full = boundary in ("submilestone", "promotion") or safety_critical
    full |= any(p.startswith(("spec/", "core/", "infra/", "generated/"))
                or p.endswith(("Cargo.toml", "Cargo.lock")) for p in paths)
    return "full" if full else "targeted"


def resume_after(text, now):
    """Accept explicit ISO or clock/IANA resets; reject any uncertain value."""
    markers = list(re.finditer(r'\b(?:reset_at|resets?(?: at)?|retry_after)\b',
                               text, re.IGNORECASE))
    if not markers or now.tzinfo is None or now.utcoffset() is None:
        return None
    dates = set()
    try:
        for marker in markers:
            tail = text[marker.end():]
            prefix = re.match(r'\s*[":= ]+\s*', tail)
            if not prefix:
                return None
            value = tail[prefix.end():]
            iso = re.match(r'(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:Z|[+-]\d\d:\d\d))'
                           r'(?=$|[\s",}])', value)
            if iso:
                reset = dt.datetime.fromisoformat(iso[1].replace("Z", "+00:00"))
            else:
                clock = re.match(r'(0?[1-9]|1[0-2]):([0-5][0-9])(am|pm) '
                                 r'\(([A-Za-z0-9_+./-]+)\)(?=$|[\s",}])',
                                 value, re.IGNORECASE)
                if not clock:
                    return None
                zone = zoneinfo.ZoneInfo(clock[4])
                hour = int(clock[1]) % 12 + (12 if clock[3].lower() == "pm" else 0)
                local_now = now.astimezone(zone)
                wall = dt.datetime.combine(local_now.date(), dt.time(hour, int(clock[2])))
                if wall < local_now.replace(tzinfo=None):
                    wall += dt.timedelta(days=1)
                # Round trips reject nonexistent times; two instants reject DST folds.
                instants = set()
                for fold in (0, 1):
                    candidate = wall.replace(tzinfo=zone, fold=fold).astimezone(dt.timezone.utc)
                    if candidate.astimezone(zone).replace(tzinfo=None) == wall:
                        instants.add(candidate)
                if len(instants) != 1:
                    return None
                reset = instants.pop()
            dates.add(reset.astimezone(dt.timezone.utc))
    except (ValueError, OverflowError, OSError, zoneinfo.ZoneInfoNotFoundError):
        return None
    if len(dates) != 1:
        return None
    reset = dates.pop()
    if reset <= now:
        return None
    return max(reset + dt.timedelta(seconds=30),
               now + dt.timedelta(seconds=MIN_RETRY_SECONDS)).isoformat()


def retry_delay(value, now):
    if not isinstance(value, str):
        return None
    try:
        stamp = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
        if stamp.tzinfo is None:
            return None
        return max(0, (stamp - now).total_seconds())
    except ValueError:
        return None
