import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export const VIDEO_RENDERER_APP_ID = 'io.github.weLayerd.GnomeVideoRenderer';
export const VIDEO_RENDERER_PATH = '/io/github/weLayerd/GnomeVideoRenderer';
export const VIDEO_RENDERER_INTERFACE = 'io.github.weLayerd.GnomeVideoRenderer';
export const VIDEO_RENDERER_WINDOW_PREFIX = `${VIDEO_RENDERER_APP_ID}:`;

function logDebug(message) {
    console.log(`[we-layerd][gnome-window-bridge-v3-hanabi-port] ${message}`);
}

export class VideoRendererController {
    constructor(extensionPath) {
        this._extensionPath = extensionPath;
        this._subprocess = null;
        this._waitCancellable = null;
        this._filePath = null;
    }

    isActive() {
        return this._subprocess !== null;
    }

    matchesWindow(metaWindow) {
        const title = metaWindow?.get_title?.() ?? '';
        return title.startsWith(VIDEO_RENDERER_WINDOW_PREFIX);
    }

    start(filePath) {
        const file = Gio.File.new_for_path(filePath);
        if (!file.query_exists(null))
            return false;

        this.stop();

        try {
            const scriptPath = GLib.build_filenamev([
                this._extensionPath,
                'renderer',
                'renderer.js',
            ]);
            const launcher = Gio.SubprocessLauncher.new(
                Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE
            );
            this._subprocess = launcher.spawnv([
                'gjs',
                '-m',
                scriptPath,
                '--file',
                filePath,
            ]);
            this._filePath = filePath;
            this._waitCancellable = new Gio.Cancellable();
            this._subprocess.wait_async(this._waitCancellable, (_proc, result) => {
                try {
                    this._subprocess?.wait_finish(result);
                } catch (error) {
                    console.error(error);
                }
                this._subprocess = null;
                this._waitCancellable = null;
            });
            logDebug(`spawned GNOME video renderer for ${filePath}`);
            return true;
        } catch (error) {
            console.error(error);
            this._subprocess = null;
            this._waitCancellable = null;
            this._filePath = null;
            return false;
        }
    }

    stop() {
        this._filePath = null;
        try {
            this._callRenderer('Stop');
        } catch (_error) {
        }

        if (this._waitCancellable) {
            this._waitCancellable.cancel();
            this._waitCancellable = null;
        }

        if (this._subprocess) {
            try {
                this._subprocess.force_exit();
            } catch (_error) {
            }
            this._subprocess = null;
        }
    }

    pause() {
        if (!this.isActive())
            return false;
        return this._callRenderer('Pause');
    }

    resume() {
        if (!this.isActive())
            return false;
        return this._callRenderer('Play');
    }

    _callRenderer(method) {
        const proxy = Gio.DBusProxy.new_sync(
            Gio.DBus.session,
            Gio.DBusProxyFlags.NONE,
            null,
            VIDEO_RENDERER_APP_ID,
            VIDEO_RENDERER_PATH,
            VIDEO_RENDERER_INTERFACE,
            null
        );
        proxy.call_sync(
            method,
            null,
            Gio.DBusCallFlags.NONE,
            1000,
            null
        );
        return true;
    }
}
