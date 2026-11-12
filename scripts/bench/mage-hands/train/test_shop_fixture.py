"""Fixture-only verification. Does not exercise a model or claim agent success."""
import importlib.util
from pathlib import Path
import re
import sys
import tempfile
import threading
import unittest
from urllib.request import Request, urlopen
from urllib.parse import urlencode
import json

spec = importlib.util.spec_from_file_location("shop_fixture", Path(__file__).with_name("shop_fixture.py"))
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)


class FakeShopTests(unittest.TestCase):
    def test_correct_cart_remove_shipping_and_exactly_once_order(self):
        with tempfile.TemporaryDirectory() as d:
            server = fixture.make_server(0, d)
            t = threading.Thread(target=server.serve_forever, daemon=True)
            t.start()
            url = f"http://127.0.0.1:{server.server_port}/"
            def get(path=""):
                return urlopen(url+path).read().decode()
            def post(path, data):
                return urlopen(Request(url+path, data=urlencode(data).encode())).read().decode()
            try:
                get("cart")
                self.assertEqual(json.loads(get("oracle"))["view"], "cart")
                post("add", {"sku": "blue-m-shirt"})
                post("add", {"sku": "cedar-bag"})
                post("quantity", {"sku": "blue-m-shirt", "quantity": 2})
                post("remove", {"sku": "cedar-bag"})
                page = post("review", {"name": "Fake Buyer", "street": "1 Fake Street", "postal": "00000", "shipping": "express"})
                key = re.search(r'name="key" value="([a-f0-9]+)"', page).group(1)
                post("submit", {"key": key, "terms": "on"})
                post("submit", {"key": key, "terms": "on"})
                state = json.loads(get("oracle"))
                self.assertEqual(len(state["orders"]), 1)
                order = state["orders"][key]
                self.assertEqual(order["cart"], {"blue-m-shirt": 2})
                self.assertEqual(order["total_cents"], 4400)
                self.assertEqual(state["cart"], {})
            finally:
                server.shutdown(); server.server_close(); t.join()

    def test_unavailable_item_and_incomplete_checkout_do_not_order(self):
        shop = fixture.oracle.FakeShop(fixture.STOCK)
        with self.assertRaises(ValueError):
            shop.add("red-m-shirt")
        shop.add("silver-mug")
        with self.assertRaises(ValueError):
            shop.submit("fake-key")
        self.assertEqual(shop.orders, {})
        self.assertEqual(shop.cart, {"silver-mug": 1})
        with self.assertRaises(ValueError):
            shop.quantity("silver-mug", -1)


if __name__ == "__main__":
    unittest.main()
