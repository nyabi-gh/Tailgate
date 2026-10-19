import { isWindows } from "./platform";

const STATIC_ORIGIN = "https://static1.e621.net/";

/**
 * The webview cannot load static1 directly (hotlink protection rejects its cross-site requests),
 * so media goes through the app's `media` scheme. WebView2 exposes custom schemes as
 * `http://<scheme>.localhost`, WKWebView as `<scheme>://localhost`.
 */
export function mediaUrl(url: string): string {
  if (!url.startsWith(STATIC_ORIGIN)) throw new Error(`not an e621 media URL: ${url}`);
  const path = url.slice(STATIC_ORIGIN.length);
  return isWindows ? `http://media.localhost/${path}` : `media://localhost/${path}`;
}
