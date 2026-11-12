"""Persisted training fixture state, independent of Mage/brain assertions.

A later primary-driver live harness can bind these transitions to browser UI
controls. This module is an oracle, never an inference-time Mage input.
"""
from dataclasses import dataclass, field


@dataclass
class FakeShop:
    stock: dict
    cart: dict = field(default_factory=dict)
    shipping: str = "standard"
    address: dict = field(default_factory=dict)
    orders: dict = field(default_factory=dict)

    def add(self, sku, quantity=1):
        if not isinstance(quantity, int) or isinstance(quantity, bool) or quantity < 1:
            raise ValueError("quantity must be a positive integer")
        if sku not in self.stock or not self.stock[sku]["available"]:
            raise ValueError("item unavailable; no silent substitution")
        self.cart[sku] = self.cart.get(sku, 0) + quantity

    def quantity(self, sku, quantity):
        if sku not in self.cart or not isinstance(quantity, int) or isinstance(quantity, bool) or quantity < 0:
            raise ValueError("invalid cart quantity")
        if quantity == 0:
            self.cart.pop(sku)
        else:
            self.cart[sku] = quantity

    def total_cents(self):
        return sum(self.stock[sku]["cents"] * n for sku, n in self.cart.items()) + (800 if self.shipping == "express" else 0)

    def submit(self, key):
        if not isinstance(key, str) or not key:
            raise ValueError("idempotency key required")
        if key in self.orders:
            return self.orders[key]
        if not self.cart or not all(self.address.get(k) for k in ("name", "street", "postal")):
            raise ValueError("cart and shipping address required")
        order = {"id": len(self.orders) + 1, "cart": dict(self.cart), "total_cents": self.total_cents(),
                 "shipping": self.shipping, "address": dict(self.address)}
        self.orders[key] = order
        self.cart.clear()
        return order


def verify_order(shop, expected_cart, expected_total, key):
    order = shop.orders.get(key)
    return len(shop.orders) == 1 and order is not None and order["cart"] == expected_cart and order["total_cents"] == expected_total
