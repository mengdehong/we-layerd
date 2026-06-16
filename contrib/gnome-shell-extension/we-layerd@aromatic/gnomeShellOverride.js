import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import {
    InjectionManager,
} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Workspace from 'resource:///org/gnome/shell/ui/workspace.js';
import * as WorkspaceThumbnail from 'resource:///org/gnome/shell/ui/workspaceThumbnail.js';

function logDebug(message) {
    console.log(`[we-layerd][gnome-window-bridge-v3-hanabi-port] ${message}`);
}

export class GnomeShellOverride {
    constructor(matchWindow) {
        this._injectionManager = new InjectionManager();
        this._matchWindow = matchWindow;
    }

    enable() {
        logDebug('gnomeShellOverride.enable()');
        const matchWindow = this._matchWindow;

        this._injectionManager.overrideMethod(
            Shell.Global.prototype,
            'get_window_actors',
            originalMethod => {
                return function (hideManaged = true) {
                    const windowActors = originalMethod.call(this);
                    if (!hideManaged)
                        return windowActors;
                    return windowActors.filter(actor => !actor?.meta_window || !matchWindow(actor.meta_window));
                };
            }
        );

        this._injectionManager.overrideMethod(
            Workspace.Workspace.prototype,
            '_isOverviewWindow',
            originalMethod => {
                return function (window) {
                    if (matchWindow(window))
                        return false;
                    return originalMethod.apply(this, [window]);
                };
            }
        );

        this._injectionManager.overrideMethod(
            WorkspaceThumbnail.WorkspaceThumbnail.prototype,
            '_isOverviewWindow',
            originalMethod => {
                return function (window) {
                    if (matchWindow(window))
                        return false;
                    return originalMethod.apply(this, [window]);
                };
            }
        );

        this._injectionManager.overrideMethod(
            Meta.Display.prototype,
            'get_tab_list',
            originalMethod => {
                return function (type, workspace) {
                    const metaWindows = originalMethod.apply(this, [type, workspace]);
                    return metaWindows.filter(metaWindow => !matchWindow(metaWindow));
                };
            }
        );

        this._injectionManager.overrideMethod(
            Shell.WindowTracker.prototype,
            'get_window_app',
            originalMethod => {
                return function (window) {
                    if (matchWindow(window))
                        return null;
                    return originalMethod.apply(this, [window]);
                };
            }
        );

        this._injectionManager.overrideMethod(
            Shell.App.prototype,
            'get_windows',
            originalMethod => {
                return function () {
                    const metaWindows = originalMethod.call(this);
                    return metaWindows.filter(metaWindow => !matchWindow(metaWindow));
                };
            }
        );

        this._injectionManager.overrideMethod(
            Shell.App.prototype,
            'get_n_windows',
            _originalMethod => {
                return function () {
                    return this.get_windows().length;
                };
            }
        );

        this._injectionManager.overrideMethod(
            Shell.AppSystem.prototype,
            'get_running',
            originalMethod => {
                return function () {
                    const runningApps = originalMethod.call(this);
                    return runningApps.filter(app => app.get_n_windows() > 0);
                };
            }
        );
    }

    disable() {
        logDebug('gnomeShellOverride.disable()');
        this._injectionManager.clear();
    }
}
