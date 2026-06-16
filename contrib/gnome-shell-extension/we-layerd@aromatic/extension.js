import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'io.github.weLayerd.Gnome';
const OBJECT_PATH = '/io/github/weLayerd/Gnome';
const IFACE_XML = `
<node>
  <interface name="io.github.weLayerd.Gnome">
    <method name="Ping">
      <arg type="s" name="version" direction="out"/>
    </method>
    <method name="RegisterWindow">
      <arg type="u" name="xid" direction="in"/>
      <arg type="u" name="pid" direction="in"/>
      <arg type="s" name="title" direction="in"/>
      <arg type="s" name="wm_class" direction="in"/>
      <arg type="b" name="accepted" direction="out"/>
    </method>
    <method name="UnregisterWindow">
      <arg type="u" name="xid" direction="in"/>
      <arg type="b" name="removed" direction="out"/>
    </method>
  </interface>
</node>`;

class ManagedWindow {
    constructor(metaWindow) {
        this._window = metaWindow;
        this._signals = [];
        this._disposed = false;
        this._refreshLaterId = 0;
        this._lowerLaterId = 0;
        this._syncing = false;

        this._signals.push(metaWindow.connect_after('raised', () => this.queueRefresh()));
        this._signals.push(metaWindow.connect('position-changed', () => this.queueRefresh()));
        this._signals.push(metaWindow.connect('size-changed', () => this.queueRefresh()));
        this._signals.push(metaWindow.connect('notify::minimized', () => {
            if (!this._window.minimized)
                return;
            this.queueRefresh();
        }));
        this._signals.push(metaWindow.connect('workspace-changed', () => this.queueRefresh()));

        this.queueRefresh();
    }

    queueRefresh(delayMs = 0) {
        if (this._disposed || !this._window || this._refreshLaterId)
            return;

        this._refreshLaterId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delayMs, () => {
            this._refreshLaterId = 0;
            this.refresh();
            return GLib.SOURCE_REMOVE;
        });
    }

    refresh() {
        if (this._disposed || !this._window || this._syncing)
            return;

        this._syncing = true;

        const monitorIndex = this._window.get_monitor();
        const monitor = Main.layoutManager.monitors[monitorIndex] ?? Main.layoutManager.primaryMonitor;
        if (!monitor) {
            this._syncing = false;
            return;
        }

        try {
            this._window.unmake_above();
            this._window.stick();
            this._window.unminimize();

            const frameRect = this._window.get_frame_rect?.();
            const needsMove = !frameRect ||
                frameRect.x !== monitor.x ||
                frameRect.y !== monitor.y ||
                frameRect.width !== monitor.width ||
                frameRect.height !== monitor.height;

            if (needsMove) {
                this._window.move_resize_frame(
                    true,
                    monitor.x,
                    monitor.y,
                    monitor.width,
                    monitor.height
                );
            }
        } finally {
            this._syncing = false;
        }

        if (this._lowerLaterId)
            GLib.source_remove(this._lowerLaterId);
        this._lowerLaterId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 50, () => {
            this._lowerLaterId = 0;
            if (!this._disposed && this._window && !this._syncing)
                this._window.lower();
            return GLib.SOURCE_REMOVE;
        });
    }

    destroy() {
        this._disposed = true;
        if (this._refreshLaterId) {
            GLib.source_remove(this._refreshLaterId);
            this._refreshLaterId = 0;
        }
        if (this._lowerLaterId) {
            GLib.source_remove(this._lowerLaterId);
            this._lowerLaterId = 0;
        }
        for (const signalId of this._signals)
            this._window.disconnect(signalId);
        this._signals = [];
        this._window = null;
    }
}

export default class WeLayerdExtension extends Extension {
    enable() {
        this._target = null;
        this._managed = null;
        this._targetUnmanagedId = 0;
        this._mapId = global.window_manager.connect_after('map', (_wm, actor) => {
            this._tryAdopt(actor.get_meta_window());
        });
        this._monitorsChangedId = Main.layoutManager.connect('monitors-changed', () => {
            if (this._managed)
                this._managed.refresh();
        });

        this._dbusImpl = Gio.DBusExportedObject.wrapJSObject(IFACE_XML, this);
        this._dbusConnection = Gio.bus_get_sync(Gio.BusType.SESSION, null);
        this._dbusImpl.export(this._dbusConnection, OBJECT_PATH);
        this._dbusNameId = Gio.bus_own_name_on_connection(
            this._dbusConnection,
            BUS_NAME,
            Gio.BusNameOwnerFlags.REPLACE,
            null,
            null
        );
    }

    disable() {
        this._clearManagedWindow();

        if (this._mapId) {
            global.window_manager.disconnect(this._mapId);
            this._mapId = 0;
        }
        if (this._monitorsChangedId) {
            Main.layoutManager.disconnect(this._monitorsChangedId);
            this._monitorsChangedId = 0;
        }
        if (this._dbusImpl) {
            this._dbusImpl.unexport();
            this._dbusImpl = null;
        }
        if (this._dbusNameId) {
            Gio.bus_unown_name(this._dbusNameId);
            this._dbusNameId = 0;
        }
        this._dbusConnection = null;

        this._target = null;
    }

    Ping() {
        return 'we-layerd-gnome-bridge-v1';
    }

    RegisterWindow(xid, pid, title, wmClass) {
        this._target = {
            xid,
            pid,
            title: title ?? '',
            wmClass: wmClass ?? '',
        };

        for (const actor of global.get_window_actors()) {
            if (this._tryAdopt(actor.get_meta_window()))
                return true;
        }
        return true;
    }

    UnregisterWindow(xid) {
        if (!this._target || this._target.xid !== xid)
            return false;

        this._clearManagedWindow();
        this._target = null;
        return true;
    }

    _tryAdopt(metaWindow) {
        if (!metaWindow || !this._target)
            return false;

        if (!this._matches(metaWindow))
            return false;

        const current = this._managed?._window;
        if (current === metaWindow) {
            this._managed.refresh();
            return true;
        }

        this._clearManagedWindow();
        this._managed = new ManagedWindow(metaWindow);
        this._targetUnmanagedId = metaWindow.connect('unmanaged', () => {
            this._clearManagedWindow();
        });
        return true;
    }

    _matches(metaWindow) {
        const pid = metaWindow.get_pid?.() ?? 0;
        const title = metaWindow.get_title?.() ?? '';
        const wmClass = metaWindow.get_wm_class?.() ?? '';

        if (this._target.pid > 0 && pid === this._target.pid)
            return true;
        if (this._target.title && title.includes(this._target.title))
            return true;
        if (this._target.wmClass && wmClass.toLowerCase().includes(this._target.wmClass.toLowerCase()))
            return true;

        if (metaWindow.get_description) {
            const description = metaWindow.get_description() ?? '';
            if (description.includes(`0x${this._target.xid.toString(16)}`))
                return true;
        }

        return false;
    }

    _clearManagedWindow() {
        if (this._targetUnmanagedId && this._managed?._window) {
            this._managed._window.disconnect(this._targetUnmanagedId);
        }
        this._targetUnmanagedId = 0;
        if (this._managed) {
            this._managed.destroy();
            this._managed = null;
        }
    }
}
