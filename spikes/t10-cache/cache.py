"""P00-T10 offline reference prototype of the Kubernetes schema cache rules.

SPIKE / PROTOTYPE -- NOT APPLICATION CODE. It exists only to make the FR-26
traversal/containment rules and the cache-set integrity rules concrete and
testable. It is stdlib-only, performs no network access and runs no
subprocess. The interface (layout, name mapping, regexes, manifest fields,
function signatures and status strings) follows spikes/P00-T10-request.md
section 2 (the "frozen interface") exactly.

Layout of one cache set (origin + revision + Kubernetes version):

    <cache_root>/<set_id>/manifest.json
    <cache_root>/<set_id>/v<MAJOR>.<MINOR>.<PATCH>-standalone-strict/<name>.json

Decisions where section 2 is silent (all fail closed):
  * "No symlink in any path component" is checked for every component from
    the set directory (inclusive) downwards; cache_root itself is trusted
    application configuration and is not walked.
  * An invalid set_id passed to verify_set/lookup is reported as
    unavailable_bad_manifest; a set directory that is not a directory is
    reported as unavailable_not_regular.
  * Any entry present in the set that is not manifest.json, a listed file or
    a parent directory of a listed file (including symlinks) is
    unavailable_extra_file.
  * Freshness: age < max_age_days is fresh; age >= max_age_days is stale.
"""

import datetime
import errno
import hashlib
import json
import os
import re
import stat

# --- frozen regexes (section 2), verbatim ---------------------------------
KIND_RE = r"^[A-Za-z][A-Za-z0-9]{0,62}$"
API_VERSION_RE = (
    r"^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?"
    r"(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*/)?"
    r"v[0-9]+(?:(?:alpha|beta)[0-9]+)?$"
)
SET_ID_RE = r"^[a-z0-9][a-z0-9._-]{0,63}$"

_KIND = re.compile(KIND_RE)
_API_VERSION = re.compile(API_VERSION_RE)
_SET_ID = re.compile(SET_ID_RE)
_K8S_VERSION = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+$")
_SHA256 = re.compile(r"^[0-9a-f]{64}$")
_COMMIT = re.compile(r"^[0-9a-f]{40}$")
_TAG = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$")
_RFC3339_UTC = re.compile(
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?Z$"
)

MANIFEST_NAME = "manifest.json"
VARIANT = "standalone-strict"
MAX_MANIFEST_BYTES = 1024 * 1024
_ORIGIN_FIELDS = ("name", "url", "publisher", "revision", "licence")
_TOP_FIELDS = (
    "manifest_version", "set_id", "origin", "kubernetes_version",
    "variant", "retrieved_at", "files",
)


# --- identifiers and name mapping -----------------------------------------
def validate_identifiers(kind, api_version):
    """True only if both identifiers are str and fully match the frozen regexes."""
    if not isinstance(kind, str) or not isinstance(api_version, str):
        return False
    return bool(_KIND.fullmatch(kind)) and bool(_API_VERSION.fullmatch(api_version))


def schema_filename(kind, api_version):
    """Map (kind, apiVersion) to the cache file name, e.g. deployment-apps-v1.json."""
    if not validate_identifiers(kind, api_version):
        raise ValueError("invalid identifier")
    if "/" in api_version:
        group, version = api_version.split("/", 1)
        suffix = "-" + group.split(".", 1)[0] + "-" + version
    else:
        suffix = "-" + api_version
    return kind.lower() + suffix + ".json"


# --- manifest -------------------------------------------------------------
def _reject_duplicates(pairs):
    obj = {}
    for key, value in pairs:
        if key in obj:
            raise ValueError("duplicate JSON key")
        obj[key] = value
    return obj


def _reject_constant(_name):
    raise ValueError("non-finite JSON constant")


def _check_rel_path(path):
    """Validate a manifest file path: relative POSIX, no '..', no empty parts."""
    if not isinstance(path, str) or not path:
        raise ValueError("bad file path")
    if "\\" in path or path.startswith("/"):
        raise ValueError("bad file path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in path):
        raise ValueError("control character in file path")
    parts = path.split("/")
    for part in parts:
        if part in ("", ".", ".."):
            raise ValueError("bad file path component")
    if parts == [MANIFEST_NAME]:
        raise ValueError("manifest must not list itself")
    return parts


def _parse_utc(value):
    """Parse an RFC 3339 UTC timestamp ending in Z; ValueError otherwise."""
    if not isinstance(value, str) or not _RFC3339_UTC.fullmatch(value):
        raise ValueError("bad timestamp")
    text = value[:-1]
    if "." in text:
        head, frac = text.split(".", 1)
        text = head + "." + (frac + "000000")[:6]
    return datetime.datetime.fromisoformat(text).replace(tzinfo=datetime.timezone.utc)


def _read_manifest_bytes(path):
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        fd = os.open(path, flags)
    except OSError as exc:
        raise ValueError("cannot open manifest") from exc
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_size > MAX_MANIFEST_BYTES:
            raise ValueError("manifest not a regular file or too large")
        data = b""
        while len(data) <= MAX_MANIFEST_BYTES:
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            data += chunk
    except OSError as exc:
        raise ValueError("cannot read manifest") from exc
    finally:
        os.close(fd)
    if len(data) > MAX_MANIFEST_BYTES:
        raise ValueError("manifest too large")
    return data


def load_manifest(set_dir):
    """Load and strictly validate <set_dir>/manifest.json; ValueError on any defect."""
    set_dir = os.fspath(set_dir)
    data = _read_manifest_bytes(os.path.join(set_dir, MANIFEST_NAME))
    try:
        text = data.decode("utf-8")
        manifest = json.loads(
            text, object_pairs_hook=_reject_duplicates, parse_constant=_reject_constant
        )
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as exc:
        raise ValueError("manifest is not valid UTF-8 JSON") from exc
    if not isinstance(manifest, dict) or set(manifest) != set(_TOP_FIELDS):
        raise ValueError("manifest fields mismatch")
    if manifest["manifest_version"] != "1":
        raise ValueError("unsupported manifest_version")
    set_id = manifest["set_id"]
    if not isinstance(set_id, str) or not _SET_ID.fullmatch(set_id):
        raise ValueError("bad set_id")
    if set_id != os.path.basename(os.path.normpath(set_dir)):
        raise ValueError("set_id does not match directory name")
    origin = manifest["origin"]
    if not isinstance(origin, dict) or set(origin) != set(_ORIGIN_FIELDS):
        raise ValueError("origin fields mismatch")
    for field in _ORIGIN_FIELDS:
        value = origin[field]
        if not isinstance(value, str) or not value or len(value) > 2048:
            raise ValueError("bad origin field")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
            raise ValueError("control character in origin field")
    revision = origin["revision"]
    if not (_COMMIT.fullmatch(revision) or _TAG.fullmatch(revision)):
        raise ValueError("bad origin revision")
    kver = manifest["kubernetes_version"]
    if not isinstance(kver, str) or not _K8S_VERSION.fullmatch(kver):
        raise ValueError("bad kubernetes_version")
    if manifest["variant"] != VARIANT:
        raise ValueError("bad variant")
    _parse_utc(manifest["retrieved_at"])
    files = manifest["files"]
    if not isinstance(files, dict) or not files:
        raise ValueError("files must be a non-empty object")
    for path, digest in files.items():
        _check_rel_path(path)
        if not isinstance(digest, str) or not _SHA256.fullmatch(digest):
            raise ValueError("bad sha256")
    return manifest


# --- set verification -----------------------------------------------------
def _sha256_regular(path):
    """Hash a file opened without following symlinks; None if not regular."""
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    fd = os.open(path, flags)
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            return None
        digest = hashlib.sha256()
        while True:
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            digest.update(chunk)
        return digest.hexdigest()
    finally:
        os.close(fd)


def _walk_status(set_dir, parts):
    """lstat each component below set_dir. Returns (status_or_None, detail)."""
    current = set_dir
    for index, part in enumerate(parts):
        current = os.path.join(current, part)
        try:
            st = os.lstat(current)
        except FileNotFoundError:
            return "unavailable_missing_file", {"path": "/".join(parts[: index + 1])}
        except OSError:
            return "unavailable_not_regular", {"path": "/".join(parts[: index + 1])}
        if stat.S_ISLNK(st.st_mode):
            return "unavailable_symlink", {"path": "/".join(parts[: index + 1])}
        last = index == len(parts) - 1
        if last and not stat.S_ISREG(st.st_mode):
            return "unavailable_not_regular", {"path": "/".join(parts)}
        if not last and not stat.S_ISDIR(st.st_mode):
            return "unavailable_not_regular", {"path": "/".join(parts[: index + 1])}
    return None, {}


def _find_extra(set_dir, files):
    """First entry in the set not accounted for by the manifest, else None."""
    dirs = {""}
    for path in files:
        parts = path.split("/")
        for i in range(1, len(parts)):
            dirs.add("/".join(parts[:i]))
    for rel_dir in sorted(dirs):
        directory = os.path.join(set_dir, rel_dir) if rel_dir else set_dir
        try:
            with os.scandir(directory) as it:
                entries = sorted(entry.name for entry in it)
        except OSError:
            return rel_dir or "."
        for name in entries:
            rel = rel_dir + "/" + name if rel_dir else name
            if rel in files or rel in dirs or rel == MANIFEST_NAME and not rel_dir:
                continue
            return rel
    return None


def _verify(cache_root, set_id):
    """Return (status, details, manifest_or_None); any failure voids the set."""
    if not isinstance(set_id, str) or not _SET_ID.fullmatch(set_id):
        return "unavailable_bad_manifest", {"reason": "invalid set_id"}, None
    set_dir = os.path.join(os.fspath(cache_root), set_id)
    try:
        st = os.lstat(set_dir)
    except FileNotFoundError:
        return "unavailable_missing_manifest", {"reason": "set directory missing"}, None
    except OSError:
        return "unavailable_not_regular", {"reason": "set directory unreadable"}, None
    if stat.S_ISLNK(st.st_mode):
        return "unavailable_symlink", {"path": set_id}, None
    if not stat.S_ISDIR(st.st_mode):
        return "unavailable_not_regular", {"path": set_id}, None

    status, detail = _walk_status(set_dir, [MANIFEST_NAME])
    if status == "unavailable_missing_file":
        return "unavailable_missing_manifest", detail, None
    if status:
        return status, detail, None
    try:
        manifest = load_manifest(set_dir)
    except ValueError as exc:
        return "unavailable_bad_manifest", {"reason": str(exc)}, None

    files = manifest["files"]
    for path in sorted(files):
        parts = path.split("/")
        status, detail = _walk_status(set_dir, parts)
        if status:
            return status, detail, None
        try:
            actual = _sha256_regular(os.path.join(set_dir, *parts))
        except OSError as exc:
            if exc.errno == errno.ELOOP:
                return "unavailable_symlink", {"path": path}, None
            return "unavailable_not_regular", {"path": path}, None
        if actual is None:
            return "unavailable_not_regular", {"path": path}, None
        if actual != files[path]:
            return "unavailable_hash_mismatch", {"path": path}, None
    extra = _find_extra(set_dir, files)
    if extra is not None:
        return "unavailable_extra_file", {"path": extra}, None
    return "available", {"files": len(files)}, manifest


def verify_set(cache_root, set_id):
    """Verify a whole cache set. Returns (status, details); never partial trust."""
    status, details, _manifest = _verify(cache_root, set_id)
    return status, details


# --- containment and lookup -----------------------------------------------
def check_containment(set_dir, candidate):
    """True iff candidate (absolute, or relative to set_dir) stays inside set_dir.

    Requires: lexical prefix is set_dir, no '.', '..' or empty components below
    it, no symlink in any existing component below set_dir, the resolved path
    inside the resolved set_dir, and, if the target exists, a regular file. A
    not-yet-existing target is contained (absence is judged by the caller).
    """
    try:
        set_dir = os.path.abspath(os.fspath(set_dir))
        candidate = os.fspath(candidate)
        if not os.path.isabs(candidate):
            candidate = os.path.join(set_dir, candidate)
        prefix = set_dir.rstrip(os.sep) + os.sep
        if not candidate.startswith(prefix):
            return False
        parts = candidate[len(prefix):].split(os.sep)
        if not parts or any(p in ("", ".", "..") for p in parts):
            return False
        current = set_dir
        for index, part in enumerate(parts):
            current = os.path.join(current, part)
            try:
                st = os.lstat(current)
            except FileNotFoundError:
                break  # not present yet: the remaining parts were checked lexically
            if stat.S_ISLNK(st.st_mode):
                return False
            if index == len(parts) - 1:
                if not stat.S_ISREG(st.st_mode):
                    return False
            elif not stat.S_ISDIR(st.st_mode):
                return False
        real_set = os.path.realpath(set_dir)
        real = os.path.realpath(candidate)
        return os.path.commonpath([real_set, real]) == real_set and real != real_set
    except (OSError, ValueError):
        return False


def lookup(cache_root, set_id, kind, api_version):
    """Resolve (kind, apiVersion) to a schema path. Returns (status, path_or_None)."""
    if not validate_identifiers(kind, api_version):
        return "invalid_identifier", None
    status, _details, manifest = _verify(cache_root, set_id)
    if status != "available":
        return "set_unavailable", None
    set_dir = os.path.join(os.fspath(cache_root), set_id)
    rel = manifest["kubernetes_version"] + "-" + VARIANT + "/" + schema_filename(kind, api_version)
    path = os.path.join(set_dir, *rel.split("/"))
    if not check_containment(set_dir, path):
        return "containment_violation", None
    if rel not in manifest["files"]:
        return "unsupported_missing_schema", None
    return "found", path


# --- freshness ------------------------------------------------------------
def freshness(manifest, now, max_age_days):
    """'fresh' | 'stale' | 'unknown' from manifest['retrieved_at'] and a policy age."""
    if not isinstance(now, datetime.datetime) or now.tzinfo is None:
        raise ValueError("now must be a timezone-aware datetime")
    if isinstance(max_age_days, bool) or not isinstance(max_age_days, (int, float)) \
            or max_age_days < 0:
        raise ValueError("max_age_days must be a non-negative number")
    try:
        retrieved = _parse_utc(manifest["retrieved_at"])
    except (KeyError, TypeError, ValueError):
        return "unknown"
    age = now.astimezone(datetime.timezone.utc) - retrieved
    if age < datetime.timedelta(0):
        return "unknown"
    if age < datetime.timedelta(days=max_age_days):
        return "fresh"
    return "stale"
