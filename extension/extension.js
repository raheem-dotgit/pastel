// Sends every clipboard change to Pastel's control socket. Wayland only tells
// the focused window about clipboard changes, but the Shell always knows.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const CLIPBOARD = St.ClipboardType.CLIPBOARD;
const enc = new TextEncoder();

function send(head, body) {
    const path = `${GLib.get_user_runtime_dir()}/pastel-${GLib.get_user_name()}.sock`;
    new Gio.SocketClient().connect_async(new Gio.UnixSocketAddress({path}), null, (c, res) => {
        let conn;
        try {
            conn = c.connect_finish(res);
        } catch (e) {
            return; // Pastel is not running
        }
        const data = new Uint8Array(head.length + body.length);
        data.set(head);
        data.set(body, head.length);
        const out = conn.get_output_stream();
        out.write_all_async(data, GLib.PRIORITY_DEFAULT, null, (o, r) => {
            try { o.write_all_finish(r); } catch (e) { /* ignore */ }
            conn.close_async(GLib.PRIORITY_DEFAULT, null, null);
        });
    });
}

export default class PastelBridge extends Extension {
    enable() {
        this._selection = global.display.get_selection();
        this._id = this._selection.connect('owner-changed', (_s, type) => {
            if (type === Meta.SelectionType.SELECTION_CLIPBOARD)
                this._read();
        });
    }

    disable() {
        this._selection.disconnect(this._id);
        this._selection = null;
    }

    _read() {
        const cb = St.Clipboard.get_default();
        const types = cb.get_mimetypes(CLIPBOARD);
        if (types.some(t => t.includes('passwordManagerHint')))
            return;
        if (types.some(t => t.startsWith('text/plain') || t === 'UTF8_STRING')) {
            cb.get_text(CLIPBOARD, (_c, text) => {
                if (text)
                    send(enc.encode('clip\n'), enc.encode(text));
            });
            return;
        }
        const img = types.find(t => t === 'image/png') ?? types.find(t => t.startsWith('image/'));
        if (img) {
            cb.get_content(CLIPBOARD, img, (_c, bytes) => {
                if (bytes && bytes.get_size() > 0)
                    send(enc.encode('image\n'), bytes.toArray());
            });
        }
    }
}
