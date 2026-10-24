"""Unit tests for the P00-T10 schema-cache prototype (spike, synthetic data only).

Run from spikes/t10-cache:  /usr/bin/python3 -I -B -m unittest -v test_cache
All data lives in tempfile.TemporaryDirectory(); nothing outside it is touched.
"""

import datetime
import hashlib
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import cache  # noqa: E402

SET_ID = "synthetic-set-1"
K8S = "v1.36.5"
VDIR = K8S + "-standalone-strict"
DEPLOY = VDIR + "/deployment-apps-v1.json"
POD = VDIR + "/pod-v1.json"
SCHEMA = b'{"type":"object"}'
UTC = datetime.timezone.utc


def sha(data):
    return hashlib.sha256(data).hexdigest()


def make_manifest(set_id=SET_ID, files=None, **overrides):
    if files is None:
        files = {DEPLOY: sha(SCHEMA), POD: sha(SCHEMA)}
    manifest = {
        "manifest_version": "1",
        "set_id": set_id,
        "origin": {
            "name": "synthetic",
            "url": "https://example.invalid/schemas",
            "publisher": "nobody",
            "revision": "0123456789abcdef0123456789abcdef01234567",
            "licence": "Apache-2.0",
        },
        "kubernetes_version": K8S,
        "variant": "standalone-strict",
        "retrieved_at": "2026-10-01T00:00:00Z",
        "files": files,
    }
    manifest.update(overrides)
    return manifest


def write(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as fh:
        fh.write(data)


def build_set(root, set_id=SET_ID, manifest=None, names=(DEPLOY, POD)):
    """Create a valid synthetic set; returns its directory."""
    set_dir = os.path.join(root, set_id)
    for name in names:
        write(os.path.join(set_dir, *name.split("/")), SCHEMA)
    if manifest is None:
        manifest = make_manifest(set_id, {n: sha(SCHEMA) for n in names})
    write(os.path.join(set_dir, "manifest.json"), json.dumps(manifest).encode())
    return set_dir


class TmpCase(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = os.path.realpath(tmp.name)
        self.set_dir = os.path.join(self.root, SET_ID)

    def status(self, set_id=SET_ID):
        return cache.verify_set(self.root, set_id)[0]


class MappingTests(unittest.TestCase):
    def test_mapping(self):
        cases = [
            ("Deployment", "apps/v1", "deployment-apps-v1.json"),
            ("Pod", "v1", "pod-v1.json"),
            ("Ingress", "networking.k8s.io/v1", "ingress-networking-v1.json"),
            ("CronJob", "batch/v1beta1", "cronjob-batch-v1beta1.json"),
            ("Widget", "example.com/v1alpha1", "widget-example-v1alpha1.json"),
        ]
        for kind, api_version, expected in cases:
            with self.subTest(kind=kind, api_version=api_version):
                self.assertTrue(cache.validate_identifiers(kind, api_version))
                self.assertEqual(cache.schema_filename(kind, api_version), expected)

    def test_frozen_regexes_verbatim(self):
        self.assertEqual(cache.KIND_RE, r"^[A-Za-z][A-Za-z0-9]{0,62}$")
        self.assertEqual(
            cache.API_VERSION_RE,
            r"^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*/)?"
            r"v[0-9]+(?:(?:alpha|beta)[0-9]+)?$",
        )

    def test_non_str_rejected(self):
        for kind, api_version in [(None, "v1"), ("Pod", None), (b"Pod", "v1"), (1, 2)]:
            with self.subTest(kind=kind, api_version=api_version):
                self.assertFalse(cache.validate_identifiers(kind, api_version))
                with self.assertRaises(ValueError):
                    cache.schema_filename(kind, api_version)

    def test_invalid_kinds(self):
        bad = ["../../escape", "a/b", "", "Kind with space", "kind\x1b", "Kïnd", "A" * 70]
        for kind in bad:
            with self.subTest(kind=kind):
                self.assertFalse(cache.validate_identifiers(kind, "apps/v1"))
                with self.assertRaises(ValueError):
                    cache.schema_filename(kind, "apps/v1")

    def test_invalid_api_versions(self):
        bad = ["apps/v1/../x", "../v1", "APPS/v1", "v1beta", "apps/v1\n", "/v1"]
        for api_version in bad:
            with self.subTest(api_version=api_version):
                self.assertFalse(cache.validate_identifiers("Deployment", api_version))
                with self.assertRaises(ValueError):
                    cache.schema_filename("Deployment", api_version)

    def test_kind_boundary_63_chars_ok(self):
        self.assertTrue(cache.validate_identifiers("A" * 63, "v1"))
        self.assertFalse(cache.validate_identifiers("A" * 64, "v1"))


class VerifySetTests(TmpCase):
    def test_available(self):
        build_set(self.root)
        status, details = cache.verify_set(self.root, SET_ID)
        self.assertEqual(status, "available")
        self.assertEqual(details, {"files": 2})

    def test_missing_manifest(self):
        build_set(self.root)
        os.remove(os.path.join(self.set_dir, "manifest.json"))
        self.assertEqual(self.status(), "unavailable_missing_manifest")

    def test_missing_set_dir(self):
        self.assertEqual(self.status(), "unavailable_missing_manifest")

    def test_malformed_json(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, "manifest.json"), b"{not json")
        self.assertEqual(self.status(), "unavailable_bad_manifest")

    def test_wrong_manifest_version(self):
        build_set(self.root, manifest=make_manifest(manifest_version="2"))
        self.assertEqual(self.status(), "unavailable_bad_manifest")

    def test_set_id_mismatch(self):
        build_set(self.root, manifest=make_manifest(set_id="other-set"))
        self.assertEqual(self.status(), "unavailable_bad_manifest")

    def test_invalid_requested_set_id(self):
        build_set(self.root)
        for bad in ["../x", "", "A", "a/b", None]:
            with self.subTest(set_id=bad):
                self.assertEqual(self.status(bad), "unavailable_bad_manifest")

    def test_manifest_path_with_dotdot(self):
        files = {DEPLOY: sha(SCHEMA), VDIR + "/../escape.json": sha(SCHEMA)}
        build_set(self.root, manifest=make_manifest(files=files))
        self.assertEqual(self.status(), "unavailable_bad_manifest")

    def test_manifest_absolute_path(self):
        files = {DEPLOY: sha(SCHEMA), "/etc/passwd": sha(SCHEMA)}
        build_set(self.root, manifest=make_manifest(files=files))
        self.assertEqual(self.status(), "unavailable_bad_manifest")

    def test_manifest_bad_paths_and_fields(self):
        h = sha(SCHEMA)
        variants = {
            "backslash": make_manifest(files={"a\\b.json": h}),
            "empty component": make_manifest(files={VDIR + "//x.json": h}),
            "dot component": make_manifest(files={VDIR + "/./x.json": h}),
            "empty files": make_manifest(files={}),
            "uppercase hash": make_manifest(files={DEPLOY: h.upper()}),
            "short hash": make_manifest(files={DEPLOY: h[:63]}),
            "non-str hash": make_manifest(files={DEPLOY: 5}),
            "lists itself": make_manifest(files={"manifest.json": h}),
            "bad k8s version": make_manifest(kubernetes_version="1.36.5"),
            "bad variant": make_manifest(variant="loose"),
            "bad timestamp": make_manifest(retrieved_at="2026-10-01T00:00:00+00:00"),
            "extra top field": make_manifest(extra="x"),
            "bad revision": make_manifest(origin={**make_manifest()["origin"], "revision": "../x"}),
            "missing origin field": make_manifest(origin={"name": "n"}),
        }
        for label, manifest in variants.items():
            with self.subTest(label):
                set_dir = os.path.join(self.root, SET_ID)
                write(os.path.join(set_dir, "manifest.json"), json.dumps(manifest).encode())
                self.assertEqual(self.status(), "unavailable_bad_manifest")
                with self.assertRaises(ValueError):
                    cache.load_manifest(set_dir)

    def test_duplicate_json_keys_rejected(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, "manifest.json"),
              b'{"manifest_version":"1","manifest_version":"1"}')
        self.assertEqual(self.status(), "unavailable_bad_manifest")

    def test_load_manifest_accepts_valid_and_tag_revision(self):
        manifest = make_manifest()
        manifest["origin"]["revision"] = "v1.36.5"
        build_set(self.root, manifest=manifest)
        self.assertEqual(cache.load_manifest(self.set_dir)["origin"]["revision"], "v1.36.5")

    def test_hash_mismatch(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, *DEPLOY.split("/")), b'{"type":"string"}')
        self.assertEqual(self.status(), "unavailable_hash_mismatch")

    def test_missing_file(self):
        build_set(self.root)
        os.remove(os.path.join(self.set_dir, *POD.split("/")))
        self.assertEqual(self.status(), "unavailable_missing_file")

    def test_extra_file(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, VDIR, "extra-v1.json"), SCHEMA)
        self.assertEqual(self.status(), "unavailable_extra_file")

    def test_extra_file_at_set_root(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, "notes.txt"), b"x")
        self.assertEqual(self.status(), "unavailable_extra_file")

    def test_extra_directory(self):
        build_set(self.root)
        os.mkdir(os.path.join(self.set_dir, VDIR, "subdir"))
        self.assertEqual(self.status(), "unavailable_extra_file")

    def test_extra_symlink_is_extra(self):
        build_set(self.root)
        os.symlink(os.path.join(self.set_dir, *POD.split("/")),
                   os.path.join(self.set_dir, VDIR, "alias-v1.json"))
        self.assertEqual(self.status(), "unavailable_extra_file")

    def test_symlinked_schema_file(self):
        build_set(self.root)
        target = os.path.join(self.root, "elsewhere.json")
        write(target, SCHEMA)  # identical content: only the symlink must trip it
        path = os.path.join(self.set_dir, *DEPLOY.split("/"))
        os.remove(path)
        os.symlink(target, path)
        self.assertEqual(self.status(), "unavailable_symlink")

    def test_symlinked_version_directory(self):
        build_set(self.root)
        real = os.path.join(self.set_dir, VDIR)
        moved = os.path.join(self.root, "moved-version-dir")
        os.rename(real, moved)
        os.symlink(moved, real)
        self.assertEqual(self.status(), "unavailable_symlink")

    def test_symlinked_set_dir(self):
        real_root = os.path.join(self.root, "real")
        build_set(real_root)
        os.symlink(os.path.join(real_root, SET_ID), self.set_dir)
        self.assertEqual(self.status(), "unavailable_symlink")

    def test_symlinked_manifest(self):
        build_set(self.root)
        manifest = os.path.join(self.set_dir, "manifest.json")
        moved = os.path.join(self.root, "manifest-copy.json")
        os.rename(manifest, moved)
        os.symlink(moved, manifest)
        self.assertEqual(self.status(), "unavailable_symlink")

    @unittest.skipUnless(hasattr(os, "mkfifo"), "mkfifo unavailable")
    def test_fifo_where_file_listed(self):
        build_set(self.root)
        path = os.path.join(self.set_dir, *DEPLOY.split("/"))
        os.remove(path)
        os.mkfifo(path)
        self.assertEqual(self.status(), "unavailable_not_regular")

    def test_directory_where_file_expected(self):
        build_set(self.root)
        path = os.path.join(self.set_dir, *DEPLOY.split("/"))
        os.remove(path)
        os.mkdir(path)
        self.assertEqual(self.status(), "unavailable_not_regular")

    def test_file_where_directory_expected(self):
        build_set(self.root)
        vdir = os.path.join(self.set_dir, VDIR)
        for name in os.listdir(vdir):
            os.remove(os.path.join(vdir, name))
        os.rmdir(vdir)
        write(vdir, b"not a directory")
        self.assertEqual(self.status(), "unavailable_not_regular")

    def test_details_have_no_file_contents(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, *DEPLOY.split("/")), b"SECRET-CONTENT-MARKER")
        _status, details = cache.verify_set(self.root, SET_ID)
        self.assertNotIn("SECRET-CONTENT-MARKER", repr(details))


class LookupTests(TmpCase):
    def test_found(self):
        build_set(self.root)
        status, path = cache.lookup(self.root, SET_ID, "Deployment", "apps/v1")
        self.assertEqual(status, "found")
        self.assertEqual(path, os.path.join(self.set_dir, VDIR, "deployment-apps-v1.json"))
        self.assertTrue(os.path.realpath(path).startswith(os.path.realpath(self.set_dir) + os.sep))
        self.assertTrue(os.path.isfile(path))

    def test_core_group_found(self):
        build_set(self.root)
        self.assertEqual(cache.lookup(self.root, SET_ID, "Pod", "v1")[0], "found")

    def test_unsupported_missing_schema(self):
        build_set(self.root)
        self.assertEqual(cache.lookup(self.root, SET_ID, "Widget", "example.com/v1alpha1"),
                         ("unsupported_missing_schema", None))

    def test_invalid_identifier_checked_first(self):
        # Set does not even exist: identifier validation must still win.
        for kind, api_version in [("../../escape", "apps/v1"), ("Deployment", "apps/v1\n"),
                                  ("", "v1"), ("Pod", "../v1"), ("Pod", "/v1")]:
            with self.subTest(kind=kind, api_version=api_version):
                self.assertEqual(cache.lookup(self.root, SET_ID, kind, api_version),
                                 ("invalid_identifier", None))

    def test_set_unavailable_when_other_file_tampered(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, *POD.split("/")), b'{"type":"string"}')
        # Deployment itself is untouched, but the whole set must be refused.
        self.assertEqual(cache.lookup(self.root, SET_ID, "Deployment", "apps/v1"),
                         ("set_unavailable", None))

    def test_set_unavailable_on_extra_file_and_missing_set(self):
        build_set(self.root)
        write(os.path.join(self.set_dir, VDIR, "extra-v1.json"), SCHEMA)
        self.assertEqual(cache.lookup(self.root, SET_ID, "Pod", "v1"), ("set_unavailable", None))
        self.assertEqual(cache.lookup(self.root, "no-such-set", "Pod", "v1"),
                         ("set_unavailable", None))

    def test_containment_helper_rejects_dotdot(self):
        build_set(self.root)
        write(os.path.join(self.root, "outside.json"), SCHEMA)
        inside = os.path.join(self.set_dir, *DEPLOY.split("/"))
        self.assertTrue(cache.check_containment(self.set_dir, inside))
        self.assertTrue(cache.check_containment(self.set_dir, DEPLOY))
        # A well-formed but absent target is contained; absence is a lookup matter.
        self.assertTrue(cache.check_containment(self.set_dir, VDIR + "/missing-v1.json"))
        crafted = [
            os.path.join(self.set_dir, VDIR, "..", "..", "outside.json"),
            "../outside.json",
            VDIR + "/../../outside.json",
            os.path.join(self.root, "outside.json"),
            self.set_dir,
            os.path.join(self.set_dir, VDIR),
        ]
        for candidate in crafted:
            with self.subTest(candidate=candidate):
                self.assertFalse(cache.check_containment(self.set_dir, candidate))

    def test_containment_helper_rejects_symlink_escape(self):
        build_set(self.root)
        outside = os.path.join(self.root, "outside.json")
        write(outside, SCHEMA)
        link = os.path.join(self.set_dir, VDIR, "link-v1.json")
        os.symlink(outside, link)
        self.assertFalse(cache.check_containment(self.set_dir, link))

    def test_sibling_prefix_not_contained(self):
        build_set(self.root)
        sibling = os.path.join(self.root, SET_ID + "-evil", "x.json")
        write(sibling, SCHEMA)
        self.assertFalse(cache.check_containment(self.set_dir, sibling))


class FreshnessTests(unittest.TestCase):
    MANIFEST = {"retrieved_at": "2026-10-01T00:00:00Z"}

    def test_fresh(self):
        now = datetime.datetime(2026, 10, 11, tzinfo=UTC)
        self.assertEqual(cache.freshness(self.MANIFEST, now, 30), "fresh")

    def test_boundary_is_stale(self):
        boundary = datetime.datetime(2026, 10, 31, tzinfo=UTC)
        self.assertEqual(cache.freshness(self.MANIFEST, boundary, 30), "stale")
        just_before = boundary - datetime.timedelta(seconds=1)
        self.assertEqual(cache.freshness(self.MANIFEST, just_before, 30), "fresh")

    def test_stale(self):
        now = datetime.datetime(2027, 1, 1, tzinfo=UTC)
        self.assertEqual(cache.freshness(self.MANIFEST, now, 30), "stale")

    def test_unknown_for_bad_timestamps(self):
        now = datetime.datetime(2026, 10, 11, tzinfo=UTC)
        for value in ["garbage", "2026-10-01", "2026-10-01T00:00:00+00:00", "", None, 5]:
            with self.subTest(value=value):
                self.assertEqual(cache.freshness({"retrieved_at": value}, now, 30), "unknown")
        self.assertEqual(cache.freshness({}, now, 30), "unknown")

    def test_unknown_for_future_timestamp(self):
        now = datetime.datetime(2026, 9, 30, 23, 59, 59, tzinfo=UTC)
        self.assertEqual(cache.freshness(self.MANIFEST, now, 30), "unknown")

    def test_naive_now_and_bad_policy_rejected(self):
        with self.assertRaises(ValueError):
            cache.freshness(self.MANIFEST, datetime.datetime(2026, 10, 11), 30)
        with self.assertRaises(ValueError):
            cache.freshness(self.MANIFEST, datetime.datetime(2026, 10, 11, tzinfo=UTC), -1)


if __name__ == "__main__":
    unittest.main()
