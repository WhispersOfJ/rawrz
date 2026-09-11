"""Tests for catalog.validate — run with: python3 -m unittest discover catalog"""
import unittest
from unittest import mock

import validate


class PortCollisionTests(unittest.TestCase):
    def test_tcp_udp_may_share_host_port(self):
        seen = {}
        a = (8080, "tcp")
        b = (8080, "udp")
        self.assertNotIn(b, seen)
        seen[a] = "x"
        self.assertIn(a, seen)

    def test_stack_ports_rejected(self):
        self.assertIn(7780, validate.STACK_PORTS)
        self.assertIn(32400, validate.STACK_PORTS)


class RetiredRegistryTests(unittest.TestCase):
    def test_lock_loads_and_contains_bazarr(self):
        names = validate.load_retired()
        self.assertIn("bazarr", names)
        self.assertIn("uptime-kuma", names)
        self.assertGreaterEqual(len(names), 25)


class SchemaTests(unittest.TestCase):
    def test_required_fields_present_in_spec(self):
        for f in ("id", "name", "category", "image", "mem_limit", "docs"):
            self.assertIn(f, validate.SCHEMA_FIELDS)

    def test_env_regex_allows_lowercase_docker_keys(self):
        self.assertTrue(validate.ENV_KEY_RE.match("FTLCONF_webserver_api_password"))
        self.assertTrue(validate.ENV_KEY_RE.match("TZ"))
        self.assertFalse(validate.ENV_KEY_RE.match("1BAD"))


if __name__ == "__main__":
    unittest.main()
