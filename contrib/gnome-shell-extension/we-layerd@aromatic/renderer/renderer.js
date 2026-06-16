#!/usr/bin/env gjs

imports.gi.versions.Gtk = '4.0';
imports.gi.versions.Gdk = '4.0';

const {Gdk, Gio, GLib, GObject, Gtk} = imports.gi;

const APP_ID = 'io.github.weLayerd.GnomeVideoRenderer';
const OBJECT_PATH = '/io/github/weLayerd/GnomeVideoRenderer';
const IFACE_XML = `
<node>
  <interface name="io.github.weLayerd.GnomeVideoRenderer">
    <method name="Ping">
      <arg type="s" name="version" direction="out"/>
    </method>
    <method name="Play"/>
    <method name="Pause"/>
    <method name="Stop"/>
  </interface>
</node>`;

const HAVE_CONTENT_FIT = Gtk.get_major_version() > 4 ||
    (Gtk.get_major_version() === 4 && Gtk.get_minor_version() >= 8);

const RendererWindow = GObject.registerClass(
    class RendererWindow extends Gtk.ApplicationWindow {
        constructor(application, monitor, monitorIndex, filePath) {
            super({
                application,
                decorated: false,
                title: `${APP_ID}:${monitorIndex}`,
            });

            const media = Gtk.MediaFile.new_for_filename(filePath);
            media.set({
                loop: true,
                muted: true,
            });
            media.connect('notify::prepared', () => {
                media.play();
            });

            const picture = new Gtk.Picture({
                paintable: media,
                can_shrink: false,
                hexpand: true,
                vexpand: true,
            });
            if (HAVE_CONTENT_FIT)
                picture.set_content_fit(Gtk.ContentFit.COVER);

            this.set_child(picture);
            this._media = media;
            this._monitor = monitor;
            this._monitorIndex = monitorIndex;

            if (monitor)
                this.fullscreen_on_monitor(monitor);
            else
                this.fullscreen();
            this.present();
            this._media.play();
        }

        setPlaying(playing) {
            if (playing)
                this._media.play();
            else
                this._media.pause();
        }
    }
);

const RendererApp = GObject.registerClass(
    class RendererApp extends Gtk.Application {
        constructor() {
            super({
                application_id: APP_ID,
                flags: Gio.ApplicationFlags.HANDLES_COMMAND_LINE,
            });

            this._filePath = null;
            this._windows = [];
            this._dbus = null;

            this.connect('startup', () => {
                this._exportDbus();
            });
            this.connect('shutdown', () => {
                this._dbus?.unexport();
                this._dbus = null;
            });
            this.connect('activate', () => {
                if (this._windows.length === 0)
                    this._buildWindows();
            });
            this.connect('command-line', (_app, commandLine) => {
                const argv = commandLine.get_arguments();
                if (!this._parseArgs(argv)) {
                    commandLine.set_exit_status(1);
                    return 0;
                }

                this.activate();
                commandLine.set_exit_status(0);
                return 0;
            });
        }

        Ping() {
            return 'we-layerd-gnome-video-renderer-v1';
        }

        Play() {
            for (const window of this._windows)
                window.setPlaying(true);
        }

        Pause() {
            for (const window of this._windows)
                window.setPlaying(false);
        }

        Stop() {
            this.quit();
        }

        _exportDbus() {
            this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE_XML, this);
            this._dbus.export(Gio.DBus.session, OBJECT_PATH);
        }

        _parseArgs(argv) {
            for (let i = 0; i < argv.length; i += 1) {
                if (argv[i] === '--file') {
                    this._filePath = argv[i + 1] ?? null;
                    i += 1;
                }
            }

            if (!this._filePath) {
                printerr('missing --file <path>');
                return false;
            }

            const file = Gio.File.new_for_path(this._filePath);
            if (!file.query_exists(null)) {
                printerr(`video file not found: ${this._filePath}`);
                return false;
            }

            return true;
        }

        _buildWindows() {
            const display = Gdk.Display.get_default();
            if (!display)
                return;

            const monitors = display.get_monitors();
            const count = monitors.get_n_items();
            if (count === 0) {
                this._windows.push(new RendererWindow(this, null, 0, this._filePath));
                return;
            }

            for (let i = 0; i < count; i += 1) {
                const monitor = monitors.get_item(i);
                this._windows.push(new RendererWindow(this, monitor, i, this._filePath));
            }
        }
    }
);

const app = new RendererApp();
app.run(ARGV);
