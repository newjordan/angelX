"""Own org.a11y.Bus on this (private) session bus and hand out AT_SPI_BUS_ADDRESS."""
import os
from gi.repository import Gio, GLib

XML = """<node>
 <interface name="org.a11y.Bus"><method name="GetAddress"><arg type="s" direction="out"/></method></interface>
 <interface name="org.a11y.Status">
  <property name="IsEnabled" type="b" access="readwrite"/>
  <property name="ScreenReaderEnabled" type="b" access="readwrite"/>
 </interface>
</node>"""
addr = os.environ["AT_SPI_BUS_ADDRESS"]
info = Gio.DBusNodeInfo.new_for_xml(XML)


def call(conn, sender, path, iface, method, params, inv):
    inv.return_value(GLib.Variant("(s)", (addr,)))


def get_prop(conn, sender, path, iface, prop):
    return GLib.Variant("b", prop == "IsEnabled")


def acquired(conn, name):
    for i in info.interfaces:
        conn.register_object("/org/a11y/bus", i, call, get_prop, None)


Gio.bus_own_name(Gio.BusType.SESSION, "org.a11y.Bus", Gio.BusNameOwnerFlags.NONE, acquired, None, None)
GLib.MainLoop().run()
